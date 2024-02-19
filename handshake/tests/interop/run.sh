#!/bin/bash

# Build the CLI example for the Rust implementation.
# Clone and build the TypeScript implementation.
# Then exchange short and long messages between them.

# NOTE: Script should be executed from the cable.rs crate root directory.

# Execute with bash:
#
# bash handshake/tests/interop/run.sh

# Expected output:
#
# -> Building Rust implementation
# -> Installing TypeScript implementation
# -> Exchanging short msg with Node.js as initiator
# -> Exchanging short msg with Rust as initiator
# -> Exchanging long msg with Node.js as initiator
# -> Exchanging long msg with Rust as initiator
# -> Cleaning-up temporary files and directories

# Requirement versions used for initial testing:
#
# cargo 1.73.0
# rustc 1.73.0
#
# nvm   0.39.x
# node 18
# tsc   5.3.3

RUST_DIR=$(pwd)
RUST_LOG=$(mktemp)
RUST_FIFO=$(mktemp -u)

NODE_DIR=$(mktemp -d)
NODE_LOG=$(mktemp)
NODE_FIFO=$(mktemp -u)

mkfifo -m 600 "$RUST_FIFO"
mkfifo -m 600 "$NODE_FIFO"

echo '-> Building Rust implementation'
#-----------------------------------------------------------------------------#
cargo build --release --examples &> /dev/null


echo '-> Installing TypeScript implementation'
#-----------------------------------------------------------------------------#
cd $NODE_DIR
if [[ ! -d cable-handshake.ts ]]; then git clone https://github.com/cabal-club/cable-handshake.ts &> /dev/null; fi
cd cable-handshake.ts
source ~/.nvm/nvm.sh
nvm use node &> /dev/null
npm install &> /dev/null
npm run build &> /dev/null
cd $RUST_DIR


echo '-> Exchanging short msg with Node.js as initiator'
#-----------------------------------------------------------------------------#
cat $RUST_FIFO | ./target/release/examples/cli 11111111111111111111111111111111 responder "In looking after your life and following the way," 2> $RUST_LOG | nc -l 1234 -w 2 > $RUST_FIFO &

cd $NODE_DIR/cable-handshake.ts
cat $NODE_FIFO | node dist/examples/cli.js 3131313131313131313131313131313131313131313131313131313131313131 initiator "gather spirit." 2> $NODE_LOG | nc 127.0.0.1 1234 -w 2 > $NODE_FIFO

grep -q "got \"In looking after your life and following the way,\"" $NODE_LOG
grep -q "Received message: gather spirit." $RUST_LOG


echo '-> Exchanging short msg with Rust as initiator'
#-----------------------------------------------------------------------------#
cat $NODE_FIFO | node dist/examples/cli.js 3131313131313131313131313131313131313131313131313131313131313131 responder "To know enough’s enough" 2> $NODE_LOG | nc -l 1234 -w 2 > $NODE_FIFO &

cd $RUST_DIR
cat $RUST_FIFO | ./target/release/examples/cli 11111111111111111111111111111111 initiator "is enough to know." 2> $RUST_LOG | nc 127.0.0.1 1234 -w 2 > $RUST_FIFO

grep -q "Received message: To know enough's enough" $RUST_LOG
grep -q "got \"is enough to know.\"" $NODE_LOG


echo '-> Exchanging long msg with Node.js as initiator'
#-----------------------------------------------------------------------------#
cat $RUST_FIFO | ./target/release/examples/cli 11111111111111111111111111111111 responder --file handshake/tests/interop/fungi.txt 2> $RUST_LOG | nc -l 1234 -w 2 > $RUST_FIFO &

cd $NODE_DIR/cable-handshake.ts
cat $NODE_FIFO | node dist/examples/cli.js 3131313131313131313131313131313131313131313131313131313131313131 initiator @$RUST_DIR/handshake/tests/interop/taoism.txt 2> $NODE_LOG | nc 127.0.0.1 1234 -w 2 > $NODE_FIFO

grep -q "Although often inconspicuous, fungi occur in every environment on Earth" $NODE_LOG
grep -q "one must place their will in harmony with the natural way of the universe" $RUST_LOG


echo '-> Exchanging long msg with Rust as initiator'
#-----------------------------------------------------------------------------#
cat $NODE_FIFO | node dist/examples/cli.js 3131313131313131313131313131313131313131313131313131313131313131 responder @$RUST_DIR/handshake/tests/interop/fungi.txt 2> $NODE_LOG | nc -l 1234 -w 2 > $NODE_FIFO &

cd $RUST_DIR
cat $RUST_FIFO | ./target/release/examples/cli 11111111111111111111111111111111 initiator --file handshake/tests/interop/taoism.txt 2> $RUST_LOG | nc 127.0.0.1 1234 -w 2 > $RUST_FIFO

grep -q "There appears to be electrical communication between fungi in word-like components" $RUST_LOG
grep -q "Taoist cosmology is cyclic—the universe is seen as being in constant change" $NODE_LOG


echo '-> Cleaning-up temporary files and directories'
#-----------------------------------------------------------------------------#
rm $RUST_LOG $RUST_FIFO
rm $NODE_LOG $NODE_FIFO
rm -rf $NODE_DIR
