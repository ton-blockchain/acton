const MIN_GRAMS_FOR_STORAGE = grams("0.05")

/// Exclusive upper bound for NFTs per batch deploy; stays below the 255-action TVM limit.
const BATCH_SIZE_LIMIT = 250
