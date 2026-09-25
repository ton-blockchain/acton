// Codes match the FunC NFT, except NotFromOwner and TooSmallRestAmount.
// Tolk 1.5 rejects duplicate enum values, so those two no longer share 401 and 402.

enum Errors {
    BatchLimitExceeded = 399
    InvalidWorkchain = 333
    NotFromAdmin = 401
    NotFromOwner = 403
    NotFromCollection = 405
    InvalidItemIndex = 402
    TooSmallRestAmount = 406
    IncorrectForwardPayload = 708
}
