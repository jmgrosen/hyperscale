// Copyright (C) Jessie Grosen 2026
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;

use embedded_io as eio;
use embedded_storage::{ReadStorage, nor_flash::NorFlash};
use esp_bootloader_esp_idf::partitions;
use esp_println::println;
use esp_storage::FlashStorage;
use generic_array::typenum;
use littlefs2::fs::Filesystem;
use slint::SharedString;

#[derive(Debug)]
struct WrappedError(littlefs2::io::Error);

impl eio::Error for WrappedError {
    fn kind(&self) -> eio::ErrorKind {
        use littlefs2::io::Error as LfsError;
        use eio::ErrorKind;

        match self.0 {
            LfsError::IO => ErrorKind::Other,
            LfsError::CORRUPTION => ErrorKind::InvalidData,
            LfsError::NO_SUCH_ENTRY => ErrorKind::NotFound,
            LfsError::ENTRY_ALREADY_EXISTED => ErrorKind::AlreadyExists,
            LfsError::PATH_NOT_DIR => ErrorKind::InvalidData,
            LfsError::PATH_IS_DIR => ErrorKind::InvalidData,
            LfsError::DIR_NOT_EMPTY => ErrorKind::InvalidData,
            LfsError::BAD_FILE_DESCRIPTOR => ErrorKind::InvalidInput,
            LfsError::FILE_TOO_BIG => ErrorKind::InvalidData,
            LfsError::INVALID => ErrorKind::InvalidInput,
            LfsError::NO_SPACE => ErrorKind::Other,
            LfsError::NO_MEMORY => ErrorKind::OutOfMemory,
            LfsError::NO_ATTRIBUTE => ErrorKind::NotFound,
            LfsError::FILENAME_TOO_LONG => ErrorKind::InvalidInput,
            _ => ErrorKind::Other,
        }
    }
}

struct WrappedFile<'a, F>(&'a F);

impl<'a, F> eio::ErrorType for WrappedFile<'a, F> {
    type Error = WrappedError;
}

impl<'a, F: littlefs2::io::Read> eio::Read for WrappedFile<'a, F> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, WrappedError> {
        self.0.read(buf).map_err(WrappedError)
    }
}

impl<'a, F: littlefs2::io::Write> eio::Write for WrappedFile<'a, F> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, WrappedError> {
        self.0.write(buf).map_err(WrappedError)
    }

    fn flush(&mut self) -> Result<(), WrappedError> {
        self.0.flush().map_err(WrappedError)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Version {
    version: u32,
}

const CURRENT_VERSION: Version = Version { version: 0 };
const VERSION_PATH: &'static littlefs2::path::Path = littlefs2::path!("/version");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Ingredient {
    name: SharedString,
    amount: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Recipe {
    name: SharedString,
    ingredients: Vec<Ingredient>,
}

pub struct FilesystemRegion<'a>(partitions::FlashRegion<'a, FlashStorage<'a>>);

impl littlefs2::driver::Storage for FilesystemRegion<'_> {
    type CACHE_SIZE = typenum::U128;
    type LOOKAHEAD_SIZE = typenum::U16;

    const READ_SIZE: usize = 4;
    const WRITE_SIZE: usize = 4;
    const BLOCK_SIZE: usize = 4096;
    // ?? how big
    const BLOCK_COUNT: usize = 256;
    const BLOCK_CYCLES: isize = 100;

    fn read(&mut self, off: usize, buf: &mut [u8]) -> littlefs2::io::Result<usize> {
        self.0.read(off as u32, buf)
            .map(|_| buf.len())
            .map_err(|_| littlefs2::io::Error::IO)
    }

    fn write(&mut self, off: usize, data: &[u8]) -> littlefs2::io::Result<usize> {
        self.0.write(off as u32, data)
            .map(|_| data.len())
            .map_err(|_| littlefs2::io::Error::IO)
    }

    fn erase(&mut self, off: usize, len: usize) -> littlefs2::io::Result<usize> {
        self.0.erase(off as u32, (off + len) as u32)
            .map(|_| len)
            .map_err(|_| littlefs2::io::Error::IO)
    }
}

fn write_current_version<'a, 'b>(fs: &Filesystem<'a, FilesystemRegion<'b>>) {
    fs.create_file_and_then(
        VERSION_PATH,
        |f| {
            postcard::to_eio(&CURRENT_VERSION, WrappedFile(f))
                .expect("serialization error while trying to write version");
            Ok(())
        },
    ).expect("io error while trying to write version");
}

fn migrate(_version_on_disk: Version) {
    println!("lmao no migration");
}

fn init_fs(region: &mut FilesystemRegion<'_>) {
    let mut alloc = Filesystem::allocate();

    let fs = if let Ok(fs) = Filesystem::mount(&mut alloc, region) {
        println!("fs looks good");
        fs
    } else {
        Filesystem::format(region)
            .expect("failed formatting!");
        println!("formatted fs");
        Filesystem::mount(&mut alloc, region)
            .expect("failed to mount even after formatting")
    };

    let mut buf = [0u8; 24];

    let version_result: Result<Result<Version, _>, _> = fs.open_file_and_then(
        VERSION_PATH,
        |f| {
            Ok(postcard::from_eio((WrappedFile(f), &mut buf[..])).map(|t| t.0))
        },
    );
    println!("version_result = {version_result:?}");

    match version_result {
        Ok(Ok(version)) => {
            migrate(version);
        }
        Ok(Err(_)) => {
            panic!("corrupted version file??");
        }
        Err(littlefs2::io::Error::NO_SUCH_ENTRY) => {
            println!("missing version file, creating");
            write_current_version(&fs);
        }
        Err(err) => {
            panic!("io error while trying to read version: {:?}", err)
        }
    }
}

pub fn find_fs_region<'a>(pt_mem: &'a mut [u8], flash: &'a mut FlashStorage<'a>) -> FilesystemRegion<'a> {
    println!("flash size = {}", flash.capacity());

    let pt = partitions::read_partition_table(flash, pt_mem).unwrap();

    for i in 0..pt.len() {
        let raw = pt.get_partition(i).unwrap();
        println!("{:?}", raw);
    }
    println!();

    let littlefs = pt
        .find_partition(partitions::PartitionType::Data(
            partitions::DataPartitionSubType::Nvs,
        ))
        .unwrap()
        .unwrap();
    let littlefs_partition = littlefs.as_embedded_storage(flash);
    let mut region = FilesystemRegion(littlefs_partition);

    init_fs(&mut region);

    region

    // let mut bytes = [0u8; 32];
    // println!("littlefs partition size = {}", littlefs_partition.capacity());
    // println!();

    // littlefs_partition
    //     .read(0, &mut bytes)
    //     .unwrap();
    // println!("read from 0: {:02x?}", &bytes[..32]);

    // bytes[0] = bytes[0].wrapping_add(1);
    // bytes[1] = bytes[1].wrapping_add(2);

    // littlefs_partition
    //     .write(0, &bytes)
    //     .unwrap();
    // println!("write to 0: {:02x?}", &bytes[..32]);
}
