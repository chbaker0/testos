#!/usr/bin/env sh

ghc -odir build -hidir build -o build/shake shake.hs
build/shake
