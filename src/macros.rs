#[macro_export]
macro_rules! try_box_future {
    ($e:expr) => {{
        match $e {
            Ok(v) => v,
            Err(e) => return Box::new(future::err(Error::from(e))),
        }
    }};
}

#[macro_export]
macro_rules! try_cont {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(_) => continue,
        }
    };
}

#[macro_export]
macro_rules! clone {
    ($n:ident: $n2:ident $($rest:tt)*) => (
        clone!(@impl , $n: $n2 $($rest)*)
    );
    ($n:ident $($rest:tt)*) => (
        clone!(@impl , $n $($rest)*)
    );
    (@impl , $n:ident: $n2:ident $($rest:tt)*) => (
        {
            #[allow(unused_mut)] let mut $n2 = $n.clone();
            clone!(@impl $($rest)*)
        }
    );
    (@impl , $n:ident $($rest:tt)*) => (
        clone!(@impl , $n: $n $($rest)*)
    );
    (@impl => move || $body:expr) => (
        move || $body
    );
    (@impl => move |$($p:tt),*| $body:expr) => (
        move |$($p),*| $body
    );
}

#[macro_export]
macro_rules! contain {
    ($expr:expr) => {
        (|| {
            $expr
        })()
    };
    (move $expr:expr) => {
        (move || {
            $expr
        })()
    };
}

#[macro_export]
macro_rules! value {
    // { }
    ({ }) => {{
        value!(@use);
        Value::Map(HashMap::new())
    }};

    // [ ]
    ([ ]) => {{
        value!(@use);
        Value::List(Vec::new())
    }};

    // { key => [ value, ... ], ... }
    ({ $key:expr => [ $($val:tt)* ], $($rest:tt)* }) => {{
        value!(@use);
        let mut map: Map = HashMap::new();
        value!(@map map; $key => [ $($val)* ], $($rest)*);
        Value::from(map)
    }};
    // { key => [ value, ... ] }
    ({ $key:expr => [ $($val:tt)* ] }) => {{
        value!({ $key => [ $($val)* ], })
    }};
    // impls
    (@map $map:ident; $key:expr => [ $($val:tt)* ], $($rest:tt)*) => {{
        value!(@map $map; $key => [ $($val)* ]);
        value!(@map $map; $($rest)*);
    }};
    (@map $map:ident; $key:expr => [ $($val:tt)* ]) => {{
        let val = value!([ $($val)* ]);
        value!(@map $map; $key => val);
    }};

    // { key => { value, ... }, ... }
    ({ $key:expr => { $($val:tt)* }, $($rest:tt)* }) => {{
        value!(@use);
        let mut map: Map = HashMap::new();
        value!(@map map; $key => { $($val)* }, $($rest)*);
        Value::from(map)
    }};
    // { key => { value, ... } }
    ({ $key:expr => { $($val:tt)* } }) => {{
        value!({ $key => { $($val)* }, })
    }};
    // impls
    (@map $map:ident; $key:expr => { $($val:tt)* }, $($rest:tt)*) => {{
        value!(@map $map; $key => { $($val)* });
        value!(@map $map; $($rest)*);
    }};
    (@map $map:ident; $key:expr => { $($val:tt)* }) => {{
        let val = value!({ $($val)* });
        value!(@map $map; $key => val);
    }};

    // { key => value, ... }
    ({ $key:expr => $val:expr, $($rest:tt)* }) => {{
        value!(@use);
        let mut map: Map = HashMap::new();
        value!(@map map; $key => $val, $($rest)*);
        Value::from(map)
    }};
    // { key => value }
    ({ $key:expr => $val:expr }) => {{
        value!({ $key => $val, })
    }};
    // impls
    (@map $map:ident; $key:expr => $val:expr, $($rest:tt)*) => {{
        value!(@map $map; $key => $val);
        value!(@map $map; $($rest)*);
    }};
    (@map $map:ident; $key:expr => $val:expr) => {{
        $map.insert(($key).into(), Spanned::from($val));
    }};
    (@map $map:ident;) => {};

    // [ [ val, ... ], ... ]
    ([ [ $($val:tt)* ], $($rest:tt)* ]) => {{
        value!(@use);
        let mut list: List = Vec::new();
        value!(@list list; [ $($val)* ], $($rest)*);
        Value::from(list)
    }};
    // [ [ val, ... ] ]
    ([ [ $($val:tt)* ] ]) => {{
        value!([ [ $val ], ])
    }};
    // impls
    (@list $list:ident; [ $($val:tt)* ], $($rest:tt)*) => {{
        value!(@list $list; [ $($val)* ]);
        value!(@list $list; $($rest)*);
    }};
    (@list $list:ident; [ $($val:tt)* ]) => {{
        let val = value!([ $($val)* ]);
        value!(@list $list; val);
    }};

    // [ { val, ... }, ... ]
    ([ { $($val:tt)* }, $($rest:tt)* ]) => {{
        value!(@use);
        let mut list: List = Vec::new();
        value!(@list list; { $($val)* }, $($rest)*);
        Value::from(list)
    }};
    // [ { val, ... } ]
    ([ { $($val:tt)* } ]) => {{
        value!([ { $val }, ])
    }};
    // impls
    (@list $list:ident; { $($val:tt)* }, $($rest:tt)*) => {{
        value!(@list $list; { $($val)* });
        value!(@list $list; $($rest)*);
    }};
    (@list $list:ident; { $($val:tt)* }) => {{
        let val = value!({ $($val)* });
        value!(@list $list; val);
    }};

    // [ val, ... ]
    ([ $val:expr, $($rest:tt)* ]) => {{
        value!(@use);
        let mut list: List = Vec::new();
        value!(@list list; $val, $($rest)*);
        Value::from(list)
    }};
    // [ val ]
    ([ $val:expr ]) => {{
        value!([ $val, ])
    }};
    // impls
    (@list $list:ident; $val:expr, $($rest:tt)*) => {{
        value!(@list $list; $val);
        value!(@list $list; $($rest)*);
    }};
    (@list $list:ident; $val:expr) => {{
        $list.push(Spanned::from($val));
    }};
    (@list $list:ident;) => {};
    (@use) => {
        use $crate::value::*;
        #[allow(unused_imports)]
        use ::std::collections::HashMap;
    };
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::rc::Rc;
    use crate::value::*;

    fn map(mut e: Vec<(&str, Spanned<Value>)>) -> Value {
        let mut r: Map = HashMap::new();
        for (k, v) in e.drain(..) {
            r.insert(k.into(), v);
        }
        r.into()
    }

    #[test]
    fn value() {
        assert_eq!(value!{{}}, map(vec![]));
        assert_eq!(value![[]], vec![].into());
        assert_eq!(value!{{ "k" => 123 }}, map(vec![("k", 123.into())]));
        assert_eq!(value!{{ "k" => true, }}, map(vec![("k", true.into())]));
        assert_eq!(value!{{ "kk".to_owned() => 123 }}, map(vec![("kk", 123.into())]));
        assert_eq!(value!{{ "k1" => "v1", "k2" => true }}, map(vec![
            ("k1", "v1".into()),
            ("k2", true.into()),
        ]));

        assert_eq!(value!{{
            "k1" => "v1",
            "k2" => [1, "2", true, {}, {"k3" => "k4",}, [], [1, "2", true,]]
        }}, map(vec![
            ("k1", "v1".into()),
            ("k2", vec![
                1.into(),
                "2".into(),
                true.into(),
                map(vec![]),
                map(vec![("k3", "k4".into())]),
                vec![].into(),
                vec![
                    1.into(),
                    "2".into(),
                    true.into(),
                ].into(),
            ].into()),
        ]));
    }

    #[test]
    fn clone() {
        fn fun0(mut f: impl FnMut() -> u32) {
            for _ in 0..2 {
                f();
            }
        }

        fn fun1(mut f: impl FnMut(u32) -> u32) {
            for _ in 0..2 {
                f(123);
            }
        }

        fn fun2(mut f: impl FnMut(u32, &str) -> u32) {
            for _ in 0..2 {
                f(123, "test");
            }
        }

        let v1 = Rc::new(10);
        let v2 = Rc::new(20);

        fun0(clone!(v1 => move || *v1 + 123));
        fun0(clone!(v1, v2 => move || *v1 + *v2));

        fun1(clone!(v1 => move |a1| *v1 + a1));
        fun2(clone!(v1 => move |a1, _a2| *v1 + a1));

        fun1(clone!(v1: renamed_v1 => move |a1| *renamed_v1 + a1));
        fun2(clone!(v1: renamed_v1 => move |a1, _a2| *renamed_v1 + a1));

        fun1(clone!(v1, v2 => move |a1| *v1 + *v2 + a1));
        fun2(clone!(v1, v2 => move |a1, _a2| *v1 + *v2 + a1));

        fun1(clone!(v1: renamed_v1, v2 => move |a1| *renamed_v1 + *v2 + a1));
        fun1(clone!(v1, v2: renamed_v2 => move |a1| *v1 + *renamed_v2 + a1));
        fun1(clone!(v1: renamed_v1, v2: renamed_v2 => move |a1| *renamed_v1 + *renamed_v2 + a1));

        fun2(clone!(v1: renamed_v1, v2 => move |a1, _a2| *renamed_v1 + *v2 + a1));
        fun2(clone!(v1, v2: renamed_v2 => move |a1, _a2| *v1 + *renamed_v2 + a1));
        fun2(clone!(v1: renamed_v1, v2: renamed_v2 => move |a1, _a2| *renamed_v1 + *renamed_v2 + a1));
    }
}