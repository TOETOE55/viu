use viu::Views;

#[derive(Views)]
#[view_as(FuckViewA)]
#[view_as(FuckViewB)]
#[view_as(FuckViewAAndB)]
struct Fuck {
    #[mut_in(FuckViewAAndB)]
    #[mut_in(FuckViewA)]
    a: i32,
    #[mut_in(FuckViewAAndB)]
    #[ref_in(FuckViewB)]
    b: String,
}

fn foo(f: &mut Fuck) {
    *FuckViewA!(f).a += 1;
    dbg!(FuckViewB!(f).b);
    let mut a_b = FuckViewAAndB!(f);
    *a_b.b += "123";
    bar(a_b.reborrow())
}

fn bar(mut a_b: FuckViewAAndB) {
    *FuckViewA!(a_b).a += 2;
    dbg!(FuckViewB!(a_b).b);
}

fn main() {
    let mut fuck = Fuck {
        a: 0,
        b: "".to_string()
    };

    foo(&mut fuck);
    println!("fuck {:?}", fuck.a);
}
