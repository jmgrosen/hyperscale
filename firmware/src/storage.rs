// Copyright (C) Jessie Grosen 2026
// SPDX-License-Identifier: MIT

/**

Filesystem layout:

 - /version: The storage version format, postcard-serialized `Version`.
 - /recipes/<sha256 hash>: A recipe, where the hash a hex digest of the SHA256
   of its postcard-serialized form.

*/

use alloc::vec::Vec;
use core::{convert::Infallible, fmt, ops::Deref};

use embedded_io::{self as eio, WriteFmtError};
use embedded_storage::{ReadStorage, nor_flash::NorFlash};
use esp_bootloader_esp_idf::partitions;
use esp_hal::sha::{Sha, Sha256, ShaAlgorithm, ShaDigest};
use esp_println::println;
use esp_storage::FlashStorage;
use nb::block;
use generic_array::typenum;
use littlefs2::{fs::Filesystem, path::Path};
use slint::{ModelRc, SharedString, VecModel};

use crate::ui;

struct WrappedError(littlefs2::io::Error);

impl fmt::Debug for WrappedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {:?}", self.0.code(), eio::Error::kind(self))
    }
}

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

struct EioHasher<'d, 'a>(ShaDigest<'d, Sha256, &'a mut Sha<'d>>);

impl<'a, 'd> eio::ErrorType for EioHasher<'d, 'a> {
    type Error = Infallible;
}

impl<'d, 'a> eio::Write for EioHasher<'d, 'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Infallible> {
        block!(self.0.update(buf)).unwrap();
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}

impl<'d, 'a> EioHasher<'d, 'a> {
    fn finish(mut self, output: &mut [u8]) {
        block!(self.0.finish(output)).unwrap();
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
        Ok(self.0.write(buf).map_err(WrappedError).expect("write"))
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
const VERSION_PATH: &'static Path = littlefs2::path!("/version");
const RECIPES_DIR_PATH: &'static Path = littlefs2::path!("/recipes/");
const ORDER_PATH: &'static Path = littlefs2::path!("/order");

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Ingredient {
    name: SharedString,
    amount: f32,
}

impl Into<ui::Ingredient> for Ingredient {
    fn into(self) -> ui::Ingredient {
        ui::Ingredient {
            name: self.name,
            amount: self.amount,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Recipe {
    name: SharedString,
    ingredients: Vec<Ingredient>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct RecipeId {
    digest: [u8; Sha256::DIGEST_LENGTH],
}

const RECIPE_PATH_PREFIX: &'static [u8] = b"/recipes/";
const RECIPE_PATH_LENGTH: usize =
    // 2 hex bytes per digest byte
    RECIPE_PATH_PREFIX.len() + Sha256::DIGEST_LENGTH * 2;
#[derive(Debug)]
struct RecipePath {
    bytes: [u8; RECIPE_PATH_LENGTH + 1], // + 1 for null byte
}

impl Deref for RecipePath {
    type Target = Path;

    fn deref(&self) -> &Path {
        Path::from_bytes_with_nul(&self.bytes[..])
            .expect("somehow made an invalid recipe path")
    }
}

impl From<RecipeId> for RecipePath {
    fn from(id: RecipeId) -> RecipePath {
        let mut bytes = [0u8; RECIPE_PATH_LENGTH + 1];
        bytes[..RECIPE_PATH_PREFIX.len()].copy_from_slice(RECIPE_PATH_PREFIX);
        let mut hex_writer = &mut bytes[RECIPE_PATH_PREFIX.len()..RECIPE_PATH_LENGTH];
        write_hex_string(&mut hex_writer, &id.digest[..])
            .expect("failed to write hex string of recipe digest");
        RecipePath { bytes }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct RecipeOrder {
    recipes: Vec<RecipeId>,
}

impl RecipeOrder {
    fn write_to_fs<'a, 'b>(&self, fs: &Filesystem<'a, FilesystemRegion<'b>>) {
        fs.create_file_and_then(
            ORDER_PATH,
            |f| {
                postcard::to_eio(self, WrappedFile(f))
                    .expect("serialization error while trying to write order");
                Ok(())
            },
        ).map_err(WrappedError).expect("io error while trying to write order");
    }
}

fn write_hex_string<W: eio::Write>(w: &mut W, buf: &[u8]) -> Result<(), WriteFmtError<W::Error>> {
    for c in buf {
        write!(w, "{:02x}", c)?;
    }
    Ok(())
}

impl Recipe {
    fn calc_id<'d>(&self, sha: &mut Sha<'d>) -> RecipeId {
        let mut hasher = EioHasher(sha.start());
        postcard::to_eio(self, &mut hasher).unwrap();
        let mut digest = [0u8; Sha256::DIGEST_LENGTH];
        hasher.finish(&mut digest[..]);
        RecipeId { digest }
    }

    fn calc_path<'d>(&self, sha: &mut Sha<'d>) -> RecipePath {
        self.calc_id(sha).into()
    }

    fn write_to_fs<'a, 'b, 'd>(
        &self,
        fs: &Filesystem<'a, FilesystemRegion<'b>>,
        sha: &mut Sha<'d>,
    ) {
        let recipe_path = self.calc_path(sha);
        println!("recipe path is {}", recipe_path.deref());

        fs.create_file_and_then(
            &recipe_path,
            |f| {
                postcard::to_eio(self, WrappedFile(f))
                    .expect("serialization error while trying to write recipe");
                Ok(())
            },
        ).map_err(WrappedError).expect("io error while trying to write recipe");
    }
}

impl Into<ui::Recipe> for Recipe {
    fn into(self) -> ui::Recipe {
        ui::Recipe {
            name: self.name,
            // TODO: this is nasty
            ingredients: ModelRc::new(
                self.ingredients
                    .into_iter()
                    .map(|i| i.into())
                    .collect::<VecModel<_>>()
            ),
        }
    }
}

fn ingredient(name: &str, amount: f32) -> Ingredient {
    Ingredient { name: name.into(), amount }
}

fn vegan_choux() -> Recipe {
    Recipe {
        name: "Vegan Choux".into(),
        ingredients: [
            ingredient("water", 0.06),
            ingredient("soy milk", 0.06),
            ingredient("vanilla extract", 0.005),
            ingredient("sugar", 0.006),
            ingredient("vegan butter", 0.028),
            ingredient("all-purpose flour", 0.065),
            ingredient("Just Egg", 0.125),
            ingredient("soy milk", 0.030),
        ].into(),
    }
}

fn vegan_creme_pat() -> Recipe {
    Recipe {
        name: "Vegan Creme Pat".into(),
        ingredients: [
            // ingredient("soy milk", 0.243),
            // ingredient("vanilla extract", 0.010),
            // ingredient("salt", 0.001),
            // ingredient("corn starch", 0.016),
            // ingredient("sugar", 0.050),
            // ingredient("Just Egg", 0.083),
            // ingredient("vegan butter", 0.042),
            ingredient("soy milk", 0.486),
            ingredient("vanilla extract", 0.020),
            ingredient("salt", 0.002),
            ingredient("corn starch", 0.032),
            ingredient("sugar", 0.100),
            ingredient("Just Egg", 0.166),
            ingredient("vegan butter", 0.084)
        ].into(),
    }
}

fn choux() -> Recipe {
    Recipe {
        name: "Choux".into(),
        ingredients: [
            ingredient("water", 0.235),
            ingredient("butter", 0.084),
            ingredient("sugar", 0.008),
            ingredient("salt", 0.002),
            ingredient("all-purpose flour", 0.128),
            ingredient("eggs", 0.200),
        ].into(),
    }
}

fn creme_pat() -> Recipe {
    Recipe {
        name: "Creme Pat".into(),
        ingredients: [
            ingredient("milk", 0.455),
            ingredient("vanilla bean", 0.001),
            ingredient("sugar", 0.115),
            ingredient("corn starch", 0.030),
            ingredient("salt", 0.001),
            ingredient("egg yolks", 0.070),
            ingredient("butter", 0.030),
        ].into(),
    }
}

fn pasta_dough() -> Recipe {
    Recipe {
        name: "Egg Pasta".into(),
        ingredients: [
            ingredient("flour", 0.255),
            ingredient("whole eggs", 0.110),
            ingredient("egg yolks", 0.070),
            ingredient("salt", 0.003),
        ].into(),
    }
}

fn poolish_bread() -> Recipe {
    Recipe {
        name: "Poolish Bread".into(),
        ingredients: [
            ingredient("flour", 0.5),
            ingredient("yeast", 0.0004),
            ingredient("water (80F)", 0.5),
            ingredient("flour", 0.5),
            ingredient("salt", 0.021),
            ingredient("yeast", 0.003),
            ingredient("water (105F)", 0.25),
        ].into(),
    }
}

fn focaccia() -> Recipe {
    Recipe {
        name: "Focaccia".into(),
        ingredients: [
            ingredient("flour", 0.5),
            ingredient("salt", 0.01),
            ingredient("yeast", 0.004),
            ingredient("water (roomtemp)", 0.4),
            ingredient("olive oil", 0.02),
            ingredient("olive oil", 0.028),
            ingredient("olive oil", 0.02),
        ].into(),
    }
}

fn kouign_amann() -> Recipe {
    Recipe {
        name: "Kouign Amann".into(),
        ingredients: [
            ingredient("flour", 0.213),
            ingredient("salt", 0.0032),
            ingredient("yeast", 0.0016),
            ingredient("water (75F)", 0.145),
            ingredient("salted butter", 0.134),
            ingredient("sugar", 0.156),
        ].into(),
    }
}

fn pie_dough() -> Recipe {
    Recipe {
        name: "Pie Dough".into(),
        ingredients: [
            ingredient("low-protein APF", 0.225),
            ingredient("sugar", 0.015),
            ingredient("salt", 0.004),
            ingredient("unsalted butter", 0.225),
            ingredient("cold tap water", 0.115),
        ].into(),
    }
}

fn butternut_pie() -> Recipe {
    Recipe {
        name: "Butternut Pie".into(),
        ingredients: [
            ingredient("butternut puree", 0.395),
            ingredient("condensed milk", 0.680),
            ingredient("light brown sugar", 0.115),
            ingredient("vanilla extract", 0.015),
            ingredient("3/2tsp ground ginger", 0.001),
            ingredient("3/2tsp ground cinnamon", 0.001),
            ingredient("1/4tsp grated nutmeg", 0.001),
            ingredient("salt", 0.001),
            ingredient("1/8tsp ground cloves", 0.001),
            ingredient("unsalted butter", 0.030),
            ingredient("eggs", 0.145),
        ].into(),
    }
}

pub fn default_recipes() -> Vec<Recipe> {
    [
        vegan_choux(),
        vegan_creme_pat(),
        choux(),
        creme_pat(),
        pasta_dough(),
        poolish_bread(),
        focaccia(),
        kouign_amann(),
        pie_dough(),
        butternut_pie(),
    ].into()
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

fn write_default_recipes<'a, 'b, 'd>(fs: &Filesystem<'a, FilesystemRegion<'b>>, sha: &mut Sha<'d>) {
    fs.create_dir_all(RECIPES_DIR_PATH).expect("failed to make recipes directory");

    let recipes = default_recipes();
    let order = RecipeOrder {
        recipes: recipes.iter().map(|r| r.calc_id(sha)).collect(),
    };

    for recipe in default_recipes() {
        recipe.write_to_fs(&fs, sha);
    }

    order.write_to_fs(fs);
}

fn init_fs<'d>(region: &mut FilesystemRegion<'_>, sha: &mut Sha<'d>) {
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

    write_default_recipes(&fs, sha);
}

pub fn find_fs_region<'a, 'd>(pt_mem: &'a mut [u8], flash: &'a mut FlashStorage<'a>, sha: &mut Sha<'d>) -> FilesystemRegion<'a> {
    println!("flash size = {}", flash.capacity());

    let pt = partitions::read_partition_table(flash, pt_mem).unwrap();

    for i in 0..pt.len() {
        let raw = pt.get_partition(i).unwrap();
        println!("{:?}", raw);
    }
    println!();

    let littlefs = pt
        .find_partition(partitions::PartitionType::Data(
            partitions::DataPartitionSubType::LittleFs,
        ))
        .unwrap()
        .unwrap();
    println!("littlefs partition has size {}", littlefs.len());
    let littlefs_partition = littlefs.as_embedded_storage(flash);
    let mut region = FilesystemRegion(littlefs_partition);

    init_fs(&mut region, sha);

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
