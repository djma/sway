script;

pub enum MyResult<T, E> {
    Ok: T,
    Err: E,
}

struct Data<T> {
    value: T,
    other: u64,
}

fn example() {
    let foo = MyResult::Ok::<Data<bool>, str[4]>(Data {
        value: true,
        other: 1,
    });
    foo.does_not_exist();
}

struct Item<I> {
    item: I,
}

fn test<A, Z>(arg: A) -> Item<Z> {
    Item { item: arg }
}

fn main() {
    let i: Item<u64> = test(true);
}
