struct Foo<'a> {
    bar: &'a str,
}
fn main() {
    let x = String::from("hello");
    let y = Foo { bar: &x };
}
