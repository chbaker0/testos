#!/usr/bin/env sh

mkdir -p out && ghc -odir out -hidir out -o out/shake shake.hs && out/shake
