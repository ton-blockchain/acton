struct (0x0f0f0001) FanoutRequest {
    queryId: uint64
    first: address
    second: address
    third: address
}

struct (0x0f0f0002) FanoutNotice {
    queryId: uint64
    branch: uint8
}

struct (0x0f0f0003) ChainRequest {
    queryId: uint64
    hops: uint8
}

struct (0x0f0f0004) ChainStep {
    queryId: uint64
    remaining: uint8
}

struct (0x0f0f0005) BinaryRequest {
    queryId: uint64
    left: address
    right: address
}

struct (0x0f0f0006) BinaryBranch {
    queryId: uint64
    branch: uint8
    target: address
}

struct (0x0f0f0007) ExternalLogRequest {
    queryId: uint64
    target: address
}

struct (0x0f0f0008) ExternalLogNotice {
    queryId: uint64
}

struct (0x0f0f0009) SingleRequest {
    queryId: uint64
}

struct (0x0f0f000a) WideFanoutRequest {
    queryId: uint64
    first: address
    second: address
    third: address
}

struct (0x0f0f000b) AsymmetricRequest {
    queryId: uint64
    chainHops: uint8
    first: address
    second: address
    final: address
}

struct (0x0f0f000c) AsymmetricStep {
    queryId: uint64
    remaining: uint8
    final: address
}

struct (0x0f0f000d) BranchingRequest {
    queryId: uint64
    depth: uint8
    left: address
    right: address
}

struct (0x0f0f000e) BranchingNode {
    queryId: uint64
    remaining: uint8
    branch: uint8
    left: address
    right: address
}

struct (0x0f0f000f) FailRequest {
    queryId: uint64
}

struct (0x0f0f0010) MixedFailureRequest {
    queryId: uint64
    okTarget: address
}

struct (0x0f0f0011) FailStep {
    queryId: uint64
}
