use allocative::Allocative;
use starlark::any::ProvidesStaticType;
use starlark::environment::{GlobalsBuilder, Module};
use starlark::eval::Evaluator;
use starlark::syntax::{AstModule, Dialect};
use starlark::values::none::NoneType;
use starlark::values::{starlark_value, Heap, NoSerialize, StarlarkValue, Value};
use starlark::{starlark_module, starlark_simple_value};
use std::cell::RefCell;
use std::fmt::{Display, Formatter};

pub fn evaluate_rule(content: &str) -> starlark::Result<()> {
    let ast = AstModule::parse("rule.star", content.to_owned(), &Dialect::Standard)?;
    let globals = GlobalsBuilder::new().with(starlark_fetch).build();
    let module = Module::new();
    let store = Store::default();
    {
        let mut eval = Evaluator::new(&module);
        eval.extra = Some(&store);
        let _ = eval.eval_module(ast, &globals)?;
    }
    for emitted in &*store.0.borrow() {
        println!("emitted: {emitted}");
    }
    Ok(())
}

// Define a store in which to accumulate JSON strings
#[derive(Debug, ProvidesStaticType, Default)]
struct Store(RefCell<Vec<String>>);

impl Store {
    fn add(&self, x: String) {
        self.0.borrow_mut().push(x)
    }
}

#[starlark_module]
fn starlark_fetch(builder: &mut GlobalsBuilder) {
    fn fetch() -> starlark::Result<Changeset> {
        Ok(Changeset {
            repository: "test".to_owned(),
        })
    }
    fn emit(x: Value, eval: &mut Evaluator) -> starlark::Result<NoneType> {
        eval.extra
            .unwrap()
            .downcast_ref::<Store>()
            .unwrap()
            .add(x.to_json()?);
        Ok(NoneType)
    }
}

#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct Changeset {
    pub repository: String,
}
starlark_simple_value!(Changeset);

impl Display for Changeset {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Changeset[name={name}]", name = self.repository)
    }
}

#[starlark_value(type = "Extr")]
impl<'v> StarlarkValue<'v> for Changeset {
    fn get_attr(&self, attribute: &str, heap: &'v Heap) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc("asdf")),
            "members" => Some(heap.alloc(vec!["a", "b"])),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use crate::evaluate_rule;

    #[test]
    fn it_works() -> starlark::Result<()> {
        let content = indoc! {"
            def my_loop(members):
                for item in members:
                    emit(item)
            emit(fetch().name)
            my_loop(fetch().members)
        "};
        evaluate_rule(content)
    }
}
