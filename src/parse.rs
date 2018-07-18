use ::nom;
use ::yaml_rust;
use ::yaml_rust::Yaml;
use ::errors::*;

#[derive(Debug, Clone, PartialEq)]
pub enum YamlPathElem {
	DownObject(String),
	DownArray(i64),
	Up,
	Root
}

pub type YamlPath = Vec<YamlPathElem>;

named!(yaml_path<&str, Result<YamlPath>>,
	do_parse!(
		path: ws!(
			pair!(
				opt!(char!('.')),
				separated_list!(
					char!('.'),
					ws!(
						alt!(
							do_parse!(n: recognize!(nom::digit) >> (n.parse::<i64>().map(|n| YamlPathElem::DownArray(n)).chain_err(|| "Failed to parse digits as number"))) |
							do_parse!(name: recognize!(nom::alphanumeric) >> (Ok(YamlPathElem::DownObject(name.to_string())))) | // FIXME YAML keys can consist of any character, properly escaped.  So we'll have to be more robust about this.
							do_parse!(tag_s!("&") >> (Ok(YamlPathElem::Up)))
						)
					)
				)
			)
		) >>
		({
			let mut ret: Result<YamlPath> = path.1.into_iter().collect(); // Convert from Vec<Result<YamlPathElem>> to Result<Vec<YamlPathElem>>
			if path.0.is_some() { ret.map(|mut p| { p.insert(0, YamlPathElem::Root); p }) }
			else { ret }
		})
	)
);

#[derive(Debug, PartialEq)]
enum Token {
	Literal(String),
	DirectSub(YamlPath),
	CondSub(YamlPath),
	InvSub(YamlPath),
	EndSub,
	KeySub(i64),
	Comment(String),
}

named_args!(template_sub<'a>(open: &str, close: &str) <&'a str, Result<Token>>,
	alt!(
		do_parse!(tag_s!(open) >> tag_s!("!--") >> text: take_until!(&format!("--{}", close)[..]) >> tag_s!("--") >> tag_s!(close) >> (Ok(Token::Comment(text.to_string())))) |
		delimited!(
			tag_s!(open),
			switch!(opt!(one_of!("#/^!?")),
				None => do_parse!(path: yaml_path >> (path.map(|p| Token::DirectSub(p)))) |
				Some('#') => do_parse!(path: yaml_path >> (path.map(|p| Token::CondSub(p)))) |
				Some('^') => do_parse!(path: yaml_path >> (path.map(|p| Token::InvSub(p)))) |
				Some('/') => do_parse!((Ok(Token::EndSub))) | // TODO How do I return Token::EndSub without this pointless do_parse?
				Some('?') => do_parse!(n: opt!(nom::digit) >> (n.map(|x| x.parse::<i64>().chain_err(|| "Failed to parse digits as number")).unwrap_or(Ok(0)).map(|x| Token::KeySub(x)))) |
				Some('!') => do_parse!(text: take_until!(close) >> (Ok(Token::Comment(text.to_string()))))
			),
			tag_s!(close)
		)
	)
);

named_args!(template_literal<'a>(open: &str) <&'a str, Result<Token>>,
	do_parse!(
		content: alt!(take_until!(open) | nom::rest_s) >> // TODO How do I ensure here that ALL input is processed?
		(Ok(Token::Literal(content.to_string())))
	)
);

named!(yaml_block<&str, Result<Vec<Yaml>>>,
	do_parse!(
		tag_s!("---\n") >>
		block: take_until_and_consume!("\n...\n") >>
		(yaml_rust::YamlLoader::load_from_str(&block).chain_err(|| "Failed to parse YAML block"))
	)
);

named_args!(document<'a>(open: &str, close: &str) <&'a str, (Option<Result<Vec<Yaml>>>, Vec<Result<Token>>)>,
	tuple!(
		opt!(yaml_block),
		many0!(
			alt!(complete!(call!(template_sub, open, close)) | complete!(call!(template_literal, open)))
		)
	)
);

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

pub fn parse_string(input: &str, open: &str, close: &str) -> Result<(Option<Vec<Yaml>>, Vec<Node>)> { // May change this to a tuple of results in the future
	let template = match document(input, open, close) { //.expect("Failed to parse template"); // FIXME Probably because Nom's error type has some dependencies on the input document.  How do I deal with this? .chain_err(|| "Failed to parse template")?;
		Ok(x) => x,
		Err(e) => bail!(format!("Parsing failed with {:?}", e)), // Temporary patch
	};
	let mut tokens = (template.1).1.into_iter().collect::<Result<Vec<Token>>>()?;
	tokens.push(Token::Literal(template.0.to_string())); // TODO This hack will go away when I figure out how to make Nom parse all input
	let tree = build_tree(&tokens).1;
	let yaml = match (template.1).0 { // TODO This feels like a hacky way of doing this
		Some(x) => Some(x?),
		None => None,
	};
	Ok((yaml, tree))
}
