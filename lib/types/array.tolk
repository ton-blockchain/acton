/// This module provides additional methods for builtin `array` type.

/// Calls [f] for every element in the array.
///
/// ```tolk
/// val nums: array<int> = [1, 2, 3];
/// nums.each(fun(x: int) {
///     println(x);
/// });
/// ```
fun array<T>.each(self, f: (T -> void)): void {
    var i = 0;
    repeat (self.size()) {
        f(self.get(i));
        i += 1;
    }
}

/// Returns a new array with [f] applied to every element.
///
/// ```tolk
/// val nums: array<int> = [1, 2, 3];
/// val doubled = nums.map<int>(fun(x: int): int { return x * 2; });
/// // [2, 4, 6]
/// ```
fun array<T>.map<U>(self, f: (T -> U)): array<U> {
    var res: array<U> = [];
    var i = 0;
    repeat (self.size()) {
        res.push(f(self.get(i)));
        i += 1;
    }
    return res;
}

/// Returns a new array containing only elements for which [f] returns `true`.
///
/// ```tolk
/// val nums: array<int> = [1, 2, 3, 4];
/// val even = nums.filter(fun(x: int): bool { return x % 2 == 0; });
/// // [2, 4]
/// ```
fun array<T>.filter(self, f: (T -> bool)): array<T> {
    var res: array<T> = [];
    var i = 0;
    repeat (self.size()) {
        val item = self.get(i);
        if (f(item)) {
            res.push(item);
        }
        i += 1;
    }
    return res;
}
