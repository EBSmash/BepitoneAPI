#!/usr/bin/env bash

for f in layers/*; do
  cmd="INSERT INTO partitions VALUES('$(basename $f)', readfile('$f'))"
  echo sqlite3 bepitone.db \"$cmd\"
  sqlite3 bepitone.db "$cmd"
done