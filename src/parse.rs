use ::nom;
use ::yaml_rust;
use ::yaml_rust::Yaml;
use ::errors::*;

const KEYCHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

#[derive(Debug, Clone, PartialEq)]
pub enum YamlPathElem {
	DownObject(String),
	DownArray(i64),
	Up,
	Root
}

pub type YamlPath = Vec<YamlPathElem>;

/*pub fn path_str(p: &YamlPath) -> String { // TODO Surely this can be done more cleanly (less format!())
	p.iter().map(|x| match x {
		YamlPathElem::DownObject(ref s) => s.to_string(),
		YamlPathElem::DownArray(n) => format!("{}", n),
		YamlPathElem::Up => "&".to_string(),
		YamlPathElem::Root => "".to_string(),
	}).fold("".to_string(), |ret, cur| format!("{}.{}", ret, cur)) // FIXME Get rid of first "."
}*/

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
							do_parse!(name: is_a!(KEYCHARS) >> (Ok(YamlPathElem::DownObject(name.to_string())))) | // FIXME YAML keys can consist of any character, properly escaped.  So we'll have to be more robust about this.
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

named!(yaml_block<&str, Option<Result<Vec<Yaml>>>>,
	opt!(
		do_parse!(
			tag_s!("---\n") >>
			block: take_until_and_consume!("\n...\n") >>
			(yaml_rust::YamlLoader::load_from_str(&block).chain_err(|| "Failed to parse YAML block"))
		)
	)
);

named_args!(template<'a>(open: &str, close: &str) <&'a str, Vec<Result<Token>>>,
	many0!(alt!(complete!(call!(template_sub, open, close)) | complete!(call!(template_literal, open))))
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

#[derive(PartialEq)]
enum ParsePhase { Start, PostYaml, Done }

pub struct Parser {
	remain: String,
	state: ParsePhase,
}

impl Parser {
	pub fn new(input: &str) -> Self {
		Parser { remain: input.to_string(), state: ParsePhase::Start }
	}
	pub fn get_yaml(&mut self) -> Result<Option<Vec<Yaml>>> {
		if self.state != ParsePhase::Start { bail!("YAML has already been retrieved"); }
		self.state = ParsePhase::PostYaml;
		match yaml_block(&self.remain.clone()) { // TODO Is this clone necessary?
			Err(e) => bail!(format!("Parsing failed with {:?}", e)), // FIXME Can't chain_err, probably because Nom's error type is holding on to the input document.  How do I deal with this?
			Ok((s, None)) => { self.remain = s.to_string(); Ok(None) },
			Ok((s, Some(x))) => { let ret = x.chain_err(|| "Failed to parse input as YAML")?; self.remain = s.to_string(); Ok(Some(ret)) },
		}
	}
	pub fn get_tpl(&mut self, open: &str, close: &str) -> Result<Vec<Node>> {
		if self.state != ParsePhase::PostYaml { bail!("This must be done immediately after retrieving YAML"); }
		self.state = ParsePhase::Done;
		match template(&self.remain, open, close) {
			Err(e) => bail!(format!("Parsing failed with {:?}", e)), // FIXME
			Ok((s, tokens)) => {
				let mut ret = tokens.into_iter().collect::<Result<Vec<Token>>>()?;
				ret.push(Token::Literal(s.to_string()));
				Ok(build_tree(&ret).1)
			},
		}
	}
}
