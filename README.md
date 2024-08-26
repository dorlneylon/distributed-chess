# Distributed BFT chess protocol example
### Overview

It is an example of [rust-libp2p](https://github.com/libp2p/rust-libp2p) & [tonic](https://github.com/hyperium/tonic) usage for decentralized gaming with simplified HotStuff consensus implementation. For Byzantine Fault Tolerance, it is required to have `3f+1` node, where `f` is the number of non-protocol compliant nodes.

The network is permissioned. For this exact implementation, it has 4 peers, though could be changed in `PEERS` constant.

### Building and running

To build front-end, open [chess](./chess) and run:

```sh
bun i
bun run dev
```

This will install dependencies and run the front.

To build core, open [core](./core), set `.env` tracing options, and run:

```sh
cargo run -- -- port <port> -- peers <multiaddr_1> <peerid_1> ... <multiaddr_n> <peerid_n>
```

### Example

<video controls>
  <source src="assets/preview.mov" type="video/mp4">
  Your browser does not support the video tag.
</video>