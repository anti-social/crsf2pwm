{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  buildInputs = [
    picotool
    zlib
  ];

  LD_LIBRARY_PATH = lib.makeLibraryPath [ zlib ];
}
