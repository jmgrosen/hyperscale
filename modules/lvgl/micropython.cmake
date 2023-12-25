# Adapted from https://github.com/kdschlosser/lv_binding_micropython/blob/9064df0589815429f31febbc42c48fa865aa1cf9/micropython.cmake
# This file is to be given as "make USER_C_MODULES=..." when building Micropython port

cmake_policy(SET CMP0118 NEW)
message("set policy!")

find_package(Python3 REQUIRED COMPONENTS Interpreter)

separate_arguments(LV_CFLAGS_ENV UNIX_COMMAND $ENV{LV_CFLAGS})
list(APPEND LV_CFLAGS ${LV_CFLAGS_ENV} -Wno-unused-function)

# if(ESP_PLATFORM)
#     include(${CMAKE_CURRENT_LIST_DIR}/driver/esp32/lcd_bus/micropython.cmake)
# endif(ESP_PLATFORM)

set(LV_BINDING_DIR "${CMAKE_CURRENT_LIST_DIR}/lv_binding_micropython")

file(GLOB_RECURSE LVGL_HEADERS ${LV_BINDING_DIR}/lvgl/src/*.h ${LV_BINDING_DIR}/lv_conf.h)

# add_custom_command(
#     OUTPUT ${CMAKE_BINARY_DIR}/lvgl.pp.c
#     COMMAND ${CMAKE_C_COMPILER} -E ${LV_CFLAGS} -I ${LV_BINDING_DIR} -I ${LV_BINDING_DIR}/pycparser/utils/fake_libc_include -DPYCPARSER=1 ${LV_BINDING_DIR}/lvgl/lvgl.h > ${CMAKE_BINARY_DIR}/lvgl.pp.c
#     DEPENDS ${LVGL_HEADERS} ${LV_BINDING_DIR}/lvgl/lvgl.h
# )
execute_process(
    COMMAND ${CMAKE_C_COMPILER} -E ${LV_CFLAGS} -I ${LV_BINDING_DIR} -I ${LV_BINDING_DIR}/pycparser/utils/fake_libc_include -DPYCPARSER=1 ${LV_BINDING_DIR}/lvgl/lvgl.h
    OUTPUT_FILE ${CMAKE_BINARY_DIR}/lvgl.pp.c
    RESULT_VARIABLE lvgl_pp_res
)
    
if(${lvgl_pp_res} GREATER "0")
    message("RESULT: ${lvgl_pp_res}")
    message( FATAL_ERROR "Failed to generate ${CMAKE_BINARY_DIR}/lv.pp.c" )
endif()

# add_custom_command(
#     OUTPUT ${CMAKE_BINARY_DIR}/lv_mp.c
#     COMMAND ${Python3_EXECUTABLE} ${LV_BINDING_DIR}/gen/gen_mpy.py -M lvgl -MP lv -MD ${CMAKE_BINARY_DIR}/lv_mpy.json -E ${CMAKE_BINARY_DIR}/lvgl.pp.c ${LV_BINDING_DIR}/lvgl/lvgl.h > ${CMAKE_BINARY_DIR}/lv_mp.c
#     DEPENDS ${LV_BINDING_DIR}/gen/gen_mpy.py ${CMAKE_BINARY_DIR}/lvgl.pp.c
#     BYPRODUCT ${CMAKE_BINARY_DIR}/lv_mpy.json
# )
execute_process(
    COMMAND ${Python3_EXECUTABLE} ${LV_BINDING_DIR}/gen/gen_mpy.py -M lvgl -MP lv -MD ${CMAKE_BINARY_DIR}/lv_mpy.json -E ${CMAKE_BINARY_DIR}/lvgl.pp.c ${LV_BINDING_DIR}/lvgl/lvgl.h
    OUTPUT_FILE ${CMAKE_BINARY_DIR}/lv_mp.c
    RESULT_VARIABLE lv_mp_res
)
    
if(${lv_mp_res} GREATER "0")
    message("RESULT: ${lv_mp_res}")
    message( FATAL_ERROR "Failed to generate ${CMAKE_BINARY_DIR}/lv_mp.c" )
endif()

# add_custom_target(generate_lvgl_mp ALL DEPENDS ${CMAKE_BINARY_DIR}/lv_mp.c)

# this MUST be an execute_process because of the order in which cmake does things
# if add_custom_command is used it errors becasue add_custom_command doesn't
# actually run before the lv_mp.c file gets added to the source list. That causes
# the compilation to error because the source file doesn't exist. It needs to
# exist before it gets added to the source list and this is the only way I have
# found to go about doing it.
# execute_process(
#     COMMAND
#         ${CMAKE_C_COMPILER} -E ${LV_CFLAGS} -I ${LV_BINDING_DIR} -I ${LV_BINDING_DIR}/pycparser/utils/fake_libc_include ${LV_BINDING_DIR}/lvgl/lvgl.h > ${CMAKE_BINARY_DIR}/lvgl.pp.c
# 	# && ${Python3_EXECUTABLE} ${LV_BINDING_DIR}/gen/gen_mpy.py -M lvgl -MP lv -MD ${CMAKE_BINARY_DIR}/lv_mpy.json -E ${CMAKE_BINARY_DIR}/lvgl.pp.c ${LV_BINDING_DIR}/lvgl/lvgl.h > ${CMAKE_BINARY_DIR}/lv_mp.c
# 	
#         # ${Python3_EXECUTABLE} ${LV_BINDING_DIR}/gen/gen_mpy.py ${LV_CFLAGS} --output=${CMAKE_BINARY_DIR}/lv_mp.c --include=${LV_BINDING_DIR} --include=${LV_BINDING_DIR}/include --include=${LV_BINDING_DIR}/lvgl --board=esp32 --module_name=lvgl --module_prefix=lv --metadata=${CMAKE_BINARY_DIR}/lv_mp.c.json ${LV_BINDING_DIR}/lvgl/lvgl.h
#     WORKING_DIRECTORY
#         ${LV_BINDING_DIR}
# 
#     RESULT_VARIABLE mpy_result
#     OUTPUT_VARIABLE mpy_output
# )

#if(${mpy_result} GREATER "0")
#    message("OUTPUT: ${mpy_output}")
#    message("RESULT: ${mpy_result}")
#    message( FATAL_ERROR "Failed to generate ${CMAKE_BINARY_DIR}/lv_mp.c" )
#endif()

# file(WRITE ${CMAKE_BINARY_DIR}/lv_mp.c ${mpy_output})

file(GLOB_RECURSE SOURCES ${LV_BINDING_DIR}/lvgl/src/*.c)

add_library(lvgl_interface INTERFACE)

target_sources(lvgl_interface INTERFACE ${SOURCES})
target_compile_options(lvgl_interface INTERFACE ${LV_CFLAGS})


set(LVGL_MPY_INCLUDES
#     ${CMAKE_CURRENT_LIST_DIR}/micropython
    ${LV_BINDING_DIR}
    ${LV_BINDING_DIR}/include
)


message("wtffffff")
add_library(usermod_lvgl INTERFACE)
# add_dependencies(usermod_lvgl generate_lvgl_mp ${CMAKE_BINARY_DIR}/lv_mp.c)
# add_dependencies(usermod_lvgl generate_lvgl_mp)
# set_source_files_properties(${CMAKE_BINARY_DIR}/lv_mp.c PROPERTIES GENERATED TRUE)
target_sources(usermod_lvgl INTERFACE ${CMAKE_BINARY_DIR}/lv_mp.c)
target_include_directories(usermod_lvgl INTERFACE ${LVGL_MPY_INCLUDES})
# target_link_libraries(usermod_lvgl INTERFACE generate_lvgl_mp)
target_link_libraries(usermod_lvgl INTERFACE lvgl_interface)
target_link_libraries(usermod INTERFACE usermod_lvgl)
