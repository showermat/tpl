#[macro_use] extern crate error_chain; // https://stevedonovan.github.io/rust-gentle-intro/6-error-handling.html#error-chain-for-serious-errors
#[macro_use] extern crate clap; // https://docs.rs/clap/2.32.0/clap/
#[macro_use] extern crate nom; // https://stevedonovan.github.io/rust-gentle-intro/nom-intro.html
extern crate yaml_rust; // http://chyh1990.github.io/yaml-rust/

use std::fs::File;
use std::io::Read;
use std::io::BufReader;
use yaml_rust::Yaml;

mod errors { error_chain!{} }
use errors::*;

// https://mustache.github.io/mustache.5.html

// TODO
// https://stackoverflow.com/questions/46876879/how-do-i-create-a-streaming-parser-in-nom
// Allow comments to contain {{ and }} -- that is, do the nesting calculation for them as well

#[derive(Debug, PartialEq)]
pub enum Token {
	Literal(String),
	DirectSub(Vec<String>),
	CondSub(Vec<String>),
	InvSub(Vec<String>),
	EndSub,
	KeySub(i64),
	Comment(String),
}

named!(yaml_path<&str, Vec<String>>,
	do_parse!(
		path: ws!(pair!(opt!(char!('.')), separated_list!(char!('.'), ws!(nom::alphanumeric)))) >> // FIXME YAML keys can consist of any character, properly escaped.  So we'll have to be more robust about this.
		({ let mut ret = path.1.iter().map(|x| x.to_string()).collect::<Vec<String>>(); if path.0.is_some() { ret.insert(0, "".to_string()); ret } else { ret } })
	)
);

named!(template_sub<&str, Token>,
	delimited!(
		tag_s!("{{"),
		switch!(opt!(one_of!("#/^!?")),
			None => do_parse!(path: yaml_path >> (Token::DirectSub(path))) |
			Some('#') => do_parse!(path: yaml_path >> (Token::CondSub(path))) |
			Some('^') => do_parse!(path: yaml_path >> (Token::InvSub(path))) |
			Some('/') => do_parse!((Token::EndSub)) | // FIXME How do I return Token::EndSub without this pointless do_parse?
			Some('?') => do_parse!(n: ws!(opt!(nom::digit)) >> (Token::KeySub(n.map(|x| x.parse::<i64>().expect("Not an integer")).unwrap_or(0)))) | // TODO I think I can do the conversion automatically with parse_to
			Some('!') => do_parse!(text: take_until!("}}") >> (Token::Comment(text.to_string())))
		),
		tag_s!("}}")
	)
);

named!(template_literal<&str, Token>,
	do_parse!(
		content: alt!(take_until!("{{") | nom::rest_s) >> // FIXME How do I ensure here that ALL input is processed?
		(Token::Literal(content.to_string()))
	)
);

named!(yaml_block<&str, Vec<Yaml>>,
	do_parse!(
		tag_s!("---\n") >>
		block: take_until_and_consume!("\n...\n") >>
		(yaml_rust::YamlLoader::load_from_str(&block).expect("Failed to parse YAML block")) // FIXME Use chain_err and return a result here rather than expecting
	)
);

named!(document<&str, (Option<Vec<Yaml>>, Vec<Token>)>,
	tuple!(
		opt!(yaml_block),
		many0!(
			alt!(complete!(template_sub) | complete!(template_literal))
		)
	)
);

fn read_file(path: &str) -> Result<String> {
	let mut ret = String::new();
	BufReader::new(File::open(path).chain_err(|| format!("Failed to open {}", path))?)
		.read_to_string(&mut ret).chain_err(|| format!("Failed to read from {}", path))?;
	Ok(ret)
}

fn yaml_pathjoin<'a>(paths: &Vec<Vec<&'a str>>) -> Vec<&'a str> {
	let mut ret = vec![];
	for elem in paths.iter().flat_map(|x| x.iter()) {
		if *elem == "" { ret.clear(); }
		else { ret.push(*elem); }
	}
	ret
}

fn yaml_get<'a>(yaml: &'a Yaml, context: &Vec<Vec<&str>>, path: &Vec<&str>) -> &'a Yaml { // TODO For these functions, should I be using &[&str], &Vec<&str>, &Vec<String>, ...?
	let mut cur = yaml;
	for elem in yaml_pathjoin(context).iter().chain(path.iter()) {
		cur = if *elem == "" { yaml } else { &cur[*elem] }
	}
	cur
}

fn yaml_bool(yaml: &Yaml) -> bool {
	match yaml {
		Yaml::BadValue | Yaml::Null | Yaml::Boolean(false) => false, // Should we also interpret 0, 0.0, and "" as falsy?
		_ => true,
	}
}

fn yaml_string(yaml: &Yaml) -> Result<String> {
	match yaml {
		Yaml::Real(x) => Ok(x.to_string()),
		Yaml::Integer(x) => Ok(x.to_string()),
		Yaml::String(x) => Ok(x.to_string()),
		Yaml::Boolean(x) => Ok(x.to_string()),
		Yaml::Null => Ok("".to_string()),
		_ => Err(Error::from("Can't stringify type")), // TODO This error message (and a lot of others) needs to be better
	}
}

#[derive(Debug)]
struct Frame {
	path: Vec<String>,
	echo: bool,
	loop_start: Option<usize>,
	loop_idx: usize,
}

fn run() -> Result<()> {
	let args = clap_app!(tpl =>
		(about: "Simple multi-purpose template engine")
		(@arg input: * index(1) "File to be templated")
		(@arg values: -f [file] "YAML file of template values")
	).get_matches();

	let input = read_file(args.value_of("input").unwrap()).chain_err(|| "Failed to get input")?;
	let template = document(&input).expect("Failed to parse template"); // FIXME Why doesn't the borrow checker like this? .chain_err(|| "Failed to parse template")?;
	let mut tokens = (template.1).1;
	let yaml = (template.1).0;
	tokens.push(Token::Literal(template.0.to_string())); // FIXME This hack will go away when I figure out how to make Nom parse all input
	let values: Vec<Yaml> = args.value_of("values")
		.map(|fname| yaml_rust::YamlLoader::load_from_str(&read_file(fname).expect("Failed to read values file")).expect("Failed to parse values file")) // FIXME Replace expects with chain_errs -- tricky inside closures
		.or_else(|| yaml)
		.ok_or(Error::from("Values are required either inline in the input or using the values flag"))?;
	let values = &values[0]; // TODO What should we do if there are multiple streams in the file?  Ignore them?

	let mut stack: Vec<Frame> = vec![];
	let mut idx = 0;
	while idx < tokens.len() {
		//println!("\x1b[31m{:?} {:?}\x1b[m", token, stack);
		let out: Result<String> = match &tokens[idx] {
			Token::Literal(s) => Ok(s.to_string()),
			Token::Comment(_) => Ok("".to_string()),
			Token::DirectSub(ref path) => yaml_string(yaml_get(values, &stack.iter().map(|x| x.path.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>(), &path.iter().map(AsRef::as_ref).collect())), // TODO Abstract the ugly map to a function or something to minimize repeated code
			Token::CondSub(ref path) => {
				let echo = yaml_bool(yaml_get(values, &stack.iter().map(|x| x.path.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>(), &path.iter().map(AsRef::as_ref).collect()));
				stack.push(Frame { path: path.to_vec(), echo: echo, loop_start: None, loop_idx: 0 });
				Ok("".to_string())
			},
			Token::InvSub(ref path) => {
				let echo = !yaml_bool(yaml_get(values, &stack.iter().map(|x| x.path.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>(), &path.iter().map(AsRef::as_ref).collect()));
				stack.push(Frame { path: path.to_vec(), echo: echo, loop_start: None, loop_idx: 0 });
				Ok("".to_string())
			},
			Token::EndSub => { stack.pop(); Ok("".to_string()) },
			Token::KeySub(n) => Ok(yaml_pathjoin(&stack.iter().map(|x| x.path.iter().map(AsRef::as_ref).collect::<Vec<&str>>()).collect::<Vec<Vec<&str>>>()).iter().rev().nth(*n as usize).map(|x| x.to_string()).ok_or(Error::from("Key level too high"))?),
		};
		print!("{}", if stack.iter().all(|x| x.echo) { out } else { Ok("".to_string()) }.chain_err(|| "Failed to template value")?);
		idx += 1;
	}
	Ok(())
}

quick_main!(run);
