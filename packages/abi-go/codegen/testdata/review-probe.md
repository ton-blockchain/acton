# Independent Review Probe

`review-probe.abi.json` preserves the ABI from the reviewer's September 9, 2026
Tolk 1.4.2 probe (reformatted only). Its relevant source was:

```tolk
struct HookBox<T> { value: T }
fun HookBox<uint8>.packToBuilder(self, mutate b: builder): void {
    b.storeUint(self.value, 16);
}
fun HookBox<uint8>.unpackFromSlice(mutate s: slice): HookBox<uint8> {
    return { value: s.loadUint(16) as uint8 };
}
type HookAlias<T> = T;
fun HookAlias<uint8>.packToBuilder(self, mutate b: builder): void {
    b.storeUint(self, 16);
}
fun HookAlias<uint8>.unpackFromSlice(mutate s: slice): HookAlias<uint8> {
    return s.loadUint(16) as HookAlias<uint8>;
}
get fun get_box(): Cell<HookBox<uint8>> {
    return HookBox<uint8> { value: 7 }.toCell();
}
get fun get_alias(): Cell<HookAlias<uint8>> {
    return (7 as HookAlias<uint8>).toCell();
}
get fun increment(v: uint8): uint8 { return v + 1; }
get fun map_slice(v: map<uint8, slice>): map<uint8, slice> { return v; }
fun onInternalMessage(in: InMessage) {}
```

The reviewer executed `increment(255)` and observed the valid TVM result 256;
the generated Fift uses `INC` with no uint8 range check. Native stack regression
tests preserve this behavior, while cell serialization still rejects 256.

The installed compiler omitted concrete hook flags, even though the generated
Fift used the custom 16-bit serializers. The TON compiler exporter at
`tolk/type-export-json.cpp:640-668` in the reviewed source emits
`custom_pack_unpack` on both concrete instantiation tables. Tests add precisely
these documented flags to the probe, exercise each flag independently, and
require all cell-dependent roots to reject them. No missing hook implementation
or layout is guessed. This small fixture is self-contained and needs no catalog
input or compiler checkout.
