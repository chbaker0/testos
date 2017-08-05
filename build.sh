#!/usr/bin/env sh

ghc -odir out -hidir out -o out/shake shake.hs
out/shake
