#[cfg(feature = "compiler")]
#[test]
fn basic() {
    use std::path::PathBuf;

    use witch::Vm;
    use witch_compiler::compile;
    use witch_runtime::value::Value;

    let expected = Value::Usize(14);
    let (bytecode, _) = compile(PathBuf::from("tests/fixtures/basic.witch"), None).unwrap();
    let mut vm = Vm::new();
    let result = vm.run(bytecode).unwrap();
    assert_eq!(expected, result);
}

#[cfg(feature = "compiler")]
#[test]
fn fib() {
    use std::path::PathBuf;

    use witch::Vm;
    use witch_compiler::compile;
    use witch_runtime::value::Value;

    let expected = Value::Usize(55);
    let (bytecode, _) = compile(PathBuf::from("tests/fixtures/fib.witch"), None).unwrap();
    let mut vm = Vm::new();
    let result = vm.run(bytecode).unwrap();
    assert_eq!(expected, result);
}

#[cfg(feature = "compiler")]
#[test]
fn lambdas() {
    use std::path::PathBuf;

    use witch::Vm;
    use witch_compiler::compile;
    use witch_runtime::value::Value;

    let expected = Value::Usize(5);
    let (bytecode, _) = compile(PathBuf::from("tests/fixtures/lambda.witch"), None).unwrap();
    let mut vm = Vm::new();
    let result = vm.run(bytecode).unwrap();
    assert_eq!(expected, result);
}

#[cfg(feature = "compiler")]
#[test]
fn closures() {
    use std::path::PathBuf;

    use witch::Vm;
    use witch_compiler::compile;
    use witch_runtime::value::Value;

    let expected = Value::Usize(14);
    let (bytecode, _) = compile(PathBuf::from("tests/fixtures/closures.witch"), None).unwrap();
    let mut vm = Vm::new();
    let result = vm.run(bytecode).unwrap();
    assert_eq!(expected, result);
}