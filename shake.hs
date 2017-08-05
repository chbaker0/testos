import Control.Monad
import Development.Shake
import Development.Shake.Command
import Development.Shake.FilePath
import Development.Shake.Util
import System.FilePath

freestanding_gcc = "i686-elf-gcc -std=gnu99 -ffreestanding -Wall -Wextra -pedantic"

-- Make a static library from objects. Needs the object files.
kernel_archive :: FilePath -> [FilePath] -> Action ()
kernel_archive target objects = do
  need objects
  cmd "ar crf" target objects

-- Assemble a NASM file for the kernel binary. Needs the source file.
kernel_assemble :: FilePath -> FilePath -> Action ()
kernel_assemble target source = do
  need [source]
  cmd "nasm -f elf32 -o" target source

-- Compile a C source for the kernel binary. Needs the source file
-- dependency file.
kernel_compile :: FilePath -> FilePath -> FilePath -> Action ()
kernel_compile target source dep = do
  need [source, dep]
  needMakefileDependencies dep
  cmd freestanding_gcc "-c -o" target source

-- Get header dependencies from a source file. Needs the source file.
kernel_dependencies :: FilePath -> FilePath -> Action ()
kernel_dependencies target source = do
  need [source]
  cmd freestanding_gcc "-MM -o" target source

build_path = "out"

extension_is ext file = ext == takeExtension file

-- |sources| can include .c, .h, and .nasm files.
static_library :: FilePath -> [FilePath] -> Rules ()
static_library name sources = do
  let sources_no_headers = filter (not . extension_is ".h") sources
  let c_sources = filter (extension_is ".c") sources_no_headers
  let nasm_sources = filter (extension_is ".nasm") sources_no_headers
  let objects = map (\x -> build_path </> (x ++ ".o")) sources_no_headers

  build_path </> name %> \target -> do
    kernel_archive name objects

  forM_ c_sources $ \source -> do
    let dep = build_path </> replaceExtension source ".c.m"
    dep %> \_ -> do
      kernel_dependencies dep source

  forM_ c_sources $ \source -> do
    let object = build_path </> replaceExtension source ".c.o"
    let dep = build_path </> replaceExtension source ".c.m"
    object %> \_ -> do
      kernel_compile object source dep

  forM_ nasm_sources $ \source -> do
    let object = build_path </> replaceExtension source ".nasm.o"
    object %> \_ -> do
      kernel_assemble object source

main :: IO ()
main = shakeArgs shakeOptions{shakeFiles = build_path} $ do
  want $ map (build_path </>) ["core.a", "cpu.a", "io.a"]

  static_library "core.a" $ map ("core/" ++) ["terminal.c",
                                              "terminal.h"]

  static_library "io.a" $ map ("io/" ++) ["vga.c",
                                          "vga.h"]

  static_library "cpu.a" $ map ("cpu/" ++) ["apic.c",
                                            "apic.h",
                                            "gdt.c",
                                            "gdt.h",
                                            "gdt.nasm",
                                            "helpers.h",
                                            "helpers.nasm",
                                            "idt.c",
                                            "idt.h",
                                            "idt.nasm",
                                            "interrupt.c",
                                            "interrupt.h",
                                            "interrupt.nasm",
                                            "pic.c",
                                            "pic.h",
                                            "port.c",
                                            "port.h"]
  -- "build/kernel.iso" %> \out -> do
  --   let sources = []
  --   ()
