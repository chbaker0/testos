import Control.Monad
import Development.Shake
import Development.Shake.Command
import Development.Shake.FilePath
import Development.Shake.Util
import System.FilePath

freestanding_gcc = "i686-elf-gcc -std=gnu99 -ffreestanding -Wall -Wextra -pedantic"

kdep = freestanding_gcc ++ " -MM -o"
kcc = freestanding_gcc ++ " -c -o"
kas = "nasm -f elf32 -o"
kar = "ar crf"

extension_is ext file = ext == takeExtension file

-- |sources| can include .c, .h, and .nasm files.
static_library :: String -> [String] -> Rules ()
static_library name sources = do
  let sources_no_headers = filter (not . extension_is ".h") sources
  let c_sources = filter (extension_is ".c") sources_no_headers
  let nasm_sources = filter (extension_is ".nasm") sources_no_headers
  let objects = map (\x -> "build" </> (x ++ ".o")) sources_no_headers

  "build" </> name %> \target -> do
    need objects
    cmd kar [target] objects

  forM_ c_sources $ \source -> do
    let dep = "build" </> replaceExtension source ".c.m"
    dep %> \_ -> do
      need [source]
      cmd kdep [dep] [source]

  forM_ c_sources $ \source -> do
    let object = "build" </> replaceExtension source ".c.o"
    let dep = "build" </> replaceExtension source ".c.m"
    object %> \_ -> do
      need [source, dep]
      needMakefileDependencies dep
      cmd kcc [object] [source] "-MMD -MF" [dep]

  forM_ nasm_sources $ \source -> do
    let object = "build" </> replaceExtension source ".nasm.o"
    object %> \_ -> do
      need [source]
      cmd kas [object] [source]

main :: IO ()
main = shakeArgs shakeOptions{shakeFiles = "build"} $ do
  want ["build/core.a", "build/cpu.a", "build/io.a"]

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
