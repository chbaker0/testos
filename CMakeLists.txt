cmake_minimum_required(VERSION 3.1)
project(testos)

set(CMAKE_ASM_NASM_OBJECT_FORMAT "elf32")
enable_language(ASM_NASM)

set(CMAKE_SYSTEM_PROCESSOR "i686")
set(CMAKE_C_COMPILER i686-elf-gcc)
set(CMAKE_C_FLAGS "-std=gnu99 -ffreestanding -Wall -Wextra -pedantic")
set(CMAKE_SHARED_LIBRARY_LINK_C_FLAGS "")
set(CMAKE_ASM_NASM_FLAGS "-g")

include_directories(.)

add_subdirectory(cpu)

add_executable(kernel.bin kernel.c console.c boot.nasm $<TARGET_OBJECTS:cpu>)

set(GRUB_PREFIX "/usr/lib/grub" CACHE PATH "Prefix of grub files")
add_custom_command(OUTPUT "kernel.iso"
  COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/iso
  COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/iso/boot
  COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_CURRENT_BINARY_DIR}/iso/boot/grub
  COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_BINARY_DIR}/kernel.bin ${CMAKE_CURRENT_BINARY_DIR}/iso/boot
  COMMAND ${CMAKE_COMMAND} -E copy ${CMAKE_CURRENT_SOURCE_DIR}/grub.cfg ${CMAKE_CURRENT_BINARY_DIR}/iso/boot/grub
  COMMAND grub-mkrescue -o kernel.iso -d ${GRUB_PREFIX}/i386-pc iso
  DEPENDS kernel.bin
  )
add_custom_target(build-kernel.iso ALL DEPENDS kernel.iso)

set_target_properties(kernel.bin PROPERTIES LINK_FLAGS "-T ${CMAKE_CURRENT_SOURCE_DIR}/linker.ld -nostdlib -lgcc")
