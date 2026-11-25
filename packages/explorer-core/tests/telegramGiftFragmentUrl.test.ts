import {expect, test} from "bun:test"

import {getTelegramGiftFragmentUrl} from "../src/nfts/telegramGiftFragmentUrl"

const giftUri = "https://nft.fragment.com/gift/mythmask-61955.json"
const giftImage = "https://nft.fragment.com/gift/mythmask-61955.webp"

test("uses the gift metadata URI to link to its Fragment page", () => {
  expect(getTelegramGiftFragmentUrl({uri: giftUri})).toBe(
    "https://fragment.com/gift/mythmask-61955",
  )
})

test.each([
  undefined,
  giftImage,
  "https://nft.fragment.com.example/gift/mythmask-61955.json",
  "https://nft.fragment.com/collection/mythmask.json",
  "https://nft.fragment.com/gift/../mythmask-61955.json",
])("ignores image fields when the metadata URI is missing or invalid (%s)", uri => {
  expect(getTelegramGiftFragmentUrl({uri, image: giftImage, image_url: giftImage})).toBeUndefined()
})
