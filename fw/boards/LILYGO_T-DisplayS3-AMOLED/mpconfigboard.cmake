set(IDF_TARGET esp32s3)

#
# Generate sdkconfig.partitions file
#
file(WRITE ${CMAKE_CURRENT_LIST_DIR}/sdkconfig.partitions
           "CONFIG_ESPTOOLPY_FLASHSIZE_16MB=y\n")
file(APPEND ${CMAKE_CURRENT_LIST_DIR}/sdkconfig.partitions
            "CONFIG_PARTITION_TABLE_CUSTOM=y\n")
file(APPEND ${CMAKE_CURRENT_LIST_DIR}/sdkconfig.partitions
            "CONFIG_PARTITION_TABLE_CUSTOM_FILENAME=\"${CMAKE_CURRENT_LIST_DIR}/partitions.csv\"\n")

set(SDKCONFIG_DEFAULTS boards/sdkconfig.base
                       boards/sdkconfig.usb
                       boards/sdkconfig.ble
                       boards/sdkconfig.240mhz
                       ${CMAKE_CURRENT_LIST_DIR}/sdkconfig.board
                       ${CMAKE_CURRENT_LIST_DIR}/sdkconfig.partitions
		       boards/sdkconfig.spiram_sx
		       boards/sdkconfig.spiram_oct
)

set(MICROPY_FROZEN_MANIFEST ${CMAKE_CURRENT_LIST_DIR}/manifest.py)

message("esp_lcd is at $ENV{IDF_PATH}/components/esp_lcd")
# list(APPEND EXTRA_COMPONENT_DIRS $ENV{IDF_PATH}/components/esp_lcd)
