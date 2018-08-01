use ::yaml_rust;
use ::parse;
use ::yaml;

fn check_render(values: Vec<&str>, template: &str, expected: &str) {
	let mut val = yaml::merge(values.into_iter().flat_map(|s| yaml_rust::YamlLoader::load_from_str(&s).unwrap().into_iter()).collect());
	let args = ::ParseArgs::from_yaml(&mut val).unwrap();
	let tpl = &parse::Parser::new(template).get_tpl(&args.open, &args.close).unwrap();
	assert_eq!(::render(&val, tpl, &vec![], args.ignore).unwrap(), expected);
}

#[test]
fn full_success() {
	//check_render(vec![], "", ""); // FIXME
	check_render(vec!["x: hi"], "{{x}}", "hi");
	check_render(vec![], "{{_config.open}}", "{{");
	check_render(vec!["x: a", "x: b"], "{{x}}", "b");
	check_render(vec!["_config:\n  open: \"{\""], "{! x }{!-- {}}\n~!@# -- (^$)(%{{&_+ --}", "");
	check_render(vec!["x: a"], "{{x}} {{.x}} {{#x}}{{}}{{/}}", "a a a");
	check_render(vec!["x:\n  a: 1\n  b: 2\n  c: 3"], "{{#x}}{{?1}}{{?}}{{}}{{/}}", "xa1xb2xc3");
	check_render(vec!["empty: []"], "{{#empty}}{{}}{{/}}{{^empty}}nothing here{{/}}", "nothing here");
	check_render(vec!["x:\n  - one\n  - two"], "{{x.0}}", "one");
	check_render(vec!["x:\n  - one"], "{{#x.0}}{{}} {{?}} {{?1}}{{/}}", "one 0 x");
	check_render(vec!["x: a\ny:\n  z: b"], "{{#x}}{{#.y}}{{}}{{?}}{{/}}{{/}}", "bz");
	let truefalse = r#"
x:
  - k: true
    t: a
    f: i
  - k: asd
    t: b
    f: j
  - k: 0
    t: c
    f: k
  - k: 0.0
    t: d
    f: l
  - k: false
    t: e
    f: m
  - k: null
    t: f
    f: n
  - k: []
    t: g
    f: o
  - k: {}
    t: h
    f: p
"#;
	check_render(vec![truefalse], "{{#x}}{{#k}}{{&.t}}{{/}}{{^k}}{{&.f}}{{/}}{{/}}", "abcdmnop");
	check_render(vec!["_config:\n  ignore: true"], "{{x}}", "");
}

#[test]
fn the_big_one() {
	let input = ::read_file("test/test.tpl").unwrap();
	let expected = ::read_file("test/test.out").unwrap();
	let mut parser = parse::Parser::new(&input);
	let val = parser.get_yaml().unwrap().unwrap().into_iter().next().unwrap();
	let tpl = parser.get_tpl("{{", "}}").unwrap();
	assert_eq!(::render(&val, &tpl, &vec![], false).unwrap(), expected);
}
