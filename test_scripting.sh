#!/home/dvk/src/rust-stargate/target/debug/stargate-shell
# Test the stargate-shell scripting language

print "=== Stargate Shell Scripting Tests ===";
print "";

print "Test 1: Variables";
let x = 5;
print x;
print "";

print "Test 2: Arithmetic";
let y = 10 + 5 * 2;
print y;
print "";

print "Test 3: Comparisons";
let a = 10;
if a > 5 {
    print "a is greater than 5";
} else {
    print "a is not greater than 5";
}
print "";

print "Test 4: Function definition and call";
fn add(a, b) {
    return a + b;
}
let sum = add(3, 4);
print sum;
print "";

print "Test 5: Recursive factorial";
fn factorial(n) {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}
let fact5 = factorial(5);
print fact5;
print "";

print "Test 6: Boolean operators";
let val = 5;
if val > 3 && val < 10 {
    print "val is between 3 and 10";
}
if val < 3 || val > 10 {
    print "val is outside 3-10 range";
} else {
    print "val is in 3-10 range";
}
print "";

print "Test 7: Calling stargate commands";
exec "echo '=== Running stargate list-directory ==='";
exec "./target/debug/stargate list-directory | head -5";
print "";

print "Test 8: Command substitution";
let greeting = "Hello from stargate-shell!";
print greeting;
print "";

print "All tests completed!";
