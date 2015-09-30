cmake_minimum_required(VERSION 3.0)
project(testos)

set(CMAKE_ASM_NASM_OBJECT_FORMAT "elf32")
enable_language(ASM_NASM)

set(CMAKE_SYSTEM_PROCESSOR "i686")
set(CMAKE_C_COMPILER i686-elf-gcc)
set(CMAKE_C_FLAGS "-std=gnu99 -ffreestanding -Wall -Wextra -pedantic")
set(CMAKE_SHARED_LIBRARY_LINK_C_FLAGS "")
set(CMAKE_ASM_NASM_FLAGS "-g")

include_directories(.)

add_subdirectory(x86)

add_executable(kernel.bin kernel.c console.c $<TARGET_OBJECTS:x86>)
set_target_properties(kernel.bin PROPERTIES LINK_FLAGS "-T ${CMAKE_CURRENT_SOURCE_DIR}/linker.ld -nostdlib -lgcc")
