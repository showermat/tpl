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
// Error if there are unmatched conds (missing {{/}}s) rather than implicitly closing them at the end
// User-friendly errors on template parsing failures
// Allow text to be included after the / in EndSub.  Either ignore it or require it to match the start text
// Options to collapse whitespace?
// All the error messages need to be a lot nicer

#[derive(Debug, Clone, PartialEq)]
pub enum YamlPathElem {
	DownObject(String),
	DownArray(i64),
	Up,
	Root
}

named!(yaml_path<&str, YamlPath>,
	do_parse!(
		path: ws!(
			pair!(
				opt!(char!('.')),
				separated_list!(
					char!('.'),
					ws!(
						alt!(
							do_parse!(n: recognize!(nom::digit) >> (YamlPathElem::DownArray(n.parse::<i64>().expect("Failed to parse digits as number")))) | // TODO Don't use expect, here or anywhere else
							do_parse!(name: recognize!(nom::alphanumeric) >> (YamlPathElem::DownObject(name.to_string()))) |
							do_parse!(tag_s!("&") >> (YamlPathElem::Up))
						)
					) // FIXME YAML keys can consist of any character, properly escaped.  So we'll have to be more robust about this.
				)
			)
		) >>
		({
			let mut ret = path.1;
			if path.0.is_some() {
				ret.insert(0, YamlPathElem::Root);
			} ret
		})
	)
);

type YamlPath = Vec<YamlPathElem>;

#[derive(Debug, PartialEq)]
pub enum Token {
	Literal(String),
	DirectSub(YamlPath),
	CondSub(YamlPath),
	InvSub(YamlPath),
	EndSub,
	KeySub(i64),
	Comment(String),
}

named!(template_sub<&str, Token>,
	delimited!(
		tag_s!("{{"),
		switch!(opt!(one_of!("#/^!?")),
			None => do_parse!(path: yaml_path >> (Token::DirectSub(path))) |
			Some('#') => do_parse!(path: yaml_path >> (Token::CondSub(path))) |
			Some('^') => do_parse!(path: yaml_path >> (Token::InvSub(path))) |
			Some('/') => do_parse!((Token::EndSub)) | // FIXME How do I return Token::EndSub without this pointless do_parse?
			Some('?') => do_parse!(n: opt!(nom::digit) >> (Token::KeySub(n.map(|x| x.parse::<i64>().expect("Failed to parse digits as number")).unwrap_or(0)))) |
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

fn yaml_pathjoin<'a>(paths: &[&YamlPath]) -> YamlPath {
	let mut ret = vec![];
	for elem in paths.iter().flat_map(|x| x.iter()) {
		match elem {
			YamlPathElem::Up => { ret.pop(); }, // It's okay if we pop an empty Vec // TODO Is this the best way to ignore the return value?
			YamlPathElem::Root => ret.clear(),
			_ => ret.push(elem.clone()), // TODO Is this clone necessary?
		};
	}
	ret
}

fn yaml_get<'a>(root: &'a Yaml, path: &YamlPath) -> &'a Yaml {
	let mut cur = root;
	let mut stack = vec![];
	for elem in path.iter() {
		cur = match elem {
			YamlPathElem::DownObject(ref key) => { stack.push(cur); &cur[&key[..]] },
			YamlPathElem::DownArray(key) => { stack.push(cur); &cur[*key as usize] },
			YamlPathElem::Up => stack.pop().unwrap_or(root),
			YamlPathElem::Root => root,
		};
	}
	cur
}

fn yaml_bool(yaml: &Yaml) -> bool {
	match yaml {
		Yaml::BadValue | Yaml::Null | Yaml::Boolean(false) => false,
		Yaml::Array(ref a) => ! a.is_empty(),
		Yaml::Hash(ref h) => ! h.is_empty(),
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
		//_ => Ok("".to_string()), // TODO Make this behavior configurable
		_ => Err(Error::from("Can't stringify type")), // TODO This error message (and a lot of others) needs to be better
	}
}

#[derive(Debug)]
pub enum Node {
	Literal(String),
	DirectSub(YamlPath),
	CondSub(YamlPath, bool, Vec<Node>),
	KeySub(i64),
}

fn build_tree(tokens: &[Token]) -> (usize, Vec<Node>) {
	let mut ret = vec![];
	let mut i: usize = 0;
	while i < tokens.len() {
		match tokens[i] {
			Token::Literal(ref s) => ret.push(Node::Literal(s.to_string())),
			Token::DirectSub(ref path) => ret.push(Node::DirectSub(path.to_vec())), // TODO Can I do this without all the to_vec()s?
			Token::CondSub(ref path) => {
				let children = build_tree(&tokens[i+1..]);
				ret.push(Node::CondSub(path.to_vec(), true, children.1));
				i += children.0 + 1;
			},
			Token::InvSub(ref path) => { // TODO Decrease duplication between CondSub and InvSub
				let children = build_tree(&tokens[i+1..]);
				ret.push(Node::CondSub(path.to_vec(), false, children.1));
				i += children.0 + 1;
			},
			Token::KeySub(n) => ret.push(Node::KeySub(n)),
			Token::EndSub => break,
			_ => (),
		};
		i += 1;
	}
	(i, ret)
}

fn render(values: &Yaml, tree: &[Node], context: &YamlPath) -> Result<String> {
	let mut ret = "".to_string();
	for node in tree {
		let cur = match node {
			Node::Literal(ref s) => s.to_string(),
			Node::DirectSub(ref path) => yaml_string(yaml_get(values, &yaml_pathjoin(&vec![context, path][..]))).chain_err(|| "Couldn't stringify value")?,
			Node::CondSub(ref path, direct, ref children) => {
				let abspath = &yaml_pathjoin(&vec![context, path][..]);
				let target = yaml_get(values, abspath);
				if yaml_bool(target) && *direct {
					match target {
						// TODO Don't unwrap; don't use as_str() (this will require doing something about the non-string key case); try to pull out common parts of these lines
						Yaml::Hash(ref contents) => contents.keys().map(|k| render(values, children, &yaml_pathjoin(&vec![abspath, &vec![YamlPathElem::DownObject(k.as_str().unwrap().to_string())]])).unwrap()).fold("".to_string(), |mut ret, cur| { ret.push_str(&cur); ret }),
						Yaml::Array(ref contents) => (0..contents.len() as i64).into_iter().map(|i| render(values, children, &yaml_pathjoin(&vec![abspath, &vec![YamlPathElem::DownArray(i)]])).unwrap()).fold("".to_string(), |mut ret, cur| { ret.push_str(&cur); ret }),
						_ => render(values, children, abspath)?,
					}
				}
				else if ! yaml_bool(target) && ! *direct {
					render(values, children, abspath)?
				}
				else { "".to_string() }
			},
			Node::KeySub(n) => match context.iter().rev().nth(*n as usize).ok_or(Error::from("No key in this context"))? {
				YamlPathElem::DownObject(ref k) => k.to_string(),
				YamlPathElem::DownArray(i) => i.to_string(),
				_ => bail!("KeySub attempted on unexpected path element"),
			},
		};
		ret.push_str(&cur);
	}
	Ok(ret)
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
	tokens.push(Token::Literal(template.0.to_string())); // FIXME This hack will go away when I figure out how to make Nom parse all input
	let tree = build_tree(&tokens).1;
	let yaml = (template.1).0;
	let values: Vec<Yaml> = args.value_of("values")
		.map(|fname| yaml_rust::YamlLoader::load_from_str(&read_file(fname).expect("Failed to read values file")).expect("Failed to parse values file")) // FIXME Replace expects with chain_errs -- tricky inside closures
		.or_else(|| yaml)
		.ok_or(Error::from("Values are required either inline in the input or using the values flag"))?;
	let values = &values[0]; // TODO What should we do if there are multiple streams in the file?  Ignore them?
	print!("{}", render(values, &tree, &vec![])?);
	Ok(())
}

quick_main!(run);
