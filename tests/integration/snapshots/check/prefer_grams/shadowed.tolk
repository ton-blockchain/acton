fun parameter(grams: int): coins { return ton("0.1"); }
fun main(): coins {
    val ton = fun(value: string): coins { return 1; };
    val grams = 2;
    return ton("0.2");
}
fun captured(): coins {
    val grams = 3;
    val callback = fun(): coins { return ton("0.3"); };
    return callback();
}
fun caught(): coins {
    try { throw 1; }
    catch (grams) { return ton("0.4"); }
}
