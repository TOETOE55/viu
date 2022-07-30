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
    *FuckViewA_ctor!(f).a += 1;
    dbg!(FuckViewB_ctor!(f).b);
    let mut a_b = FuckViewAAndB_ctor!(f);
    *a_b.b += "123";
    bar(a_b.reborrow())
}

fn bar(mut a_b: FuckViewAAndB) {
    *FuckViewA_ctor!(a_b).a += 2;
    dbg!(FuckViewB_ctor!(a_b).b);
}


mod inner {

    use super::{Fuck, FuckViewB};
    #[allow(dead_code)]
    fn assert_expend_macro() {
        let fuck = Fuck {
            a: 0,
            b: "".to_string()
        };

        FuckViewB_ctor!(fuck);
    }
}

fn main() {
    let mut fuck = Fuck {
        a: 0,
        b: "".to_string()
    };

    foo(&mut fuck);
    println!("fuck {:?}", fuck.a);
}
