use super::Parser;
use vox_ast::types::TypeExpr;
use vox_lexer::token::Token;

impl Parser {
    pub(crate) fn parse_type_expr(&mut self) -> Result<TypeExpr, ()> {
        let mut types = vec![self.parse_intersection_type_expr()?];
        while self.eat(&Token::Bar) {
            types.push(self.parse_intersection_type_expr()?);
        }
        if types.len() == 1 {
            Ok(types.into_iter().next().ok_or(())?)
        } else {
            let (first, last) = (types.first().ok_or(())?, types.last().ok_or(())?);
            let span = first.span().merge(last.span());
            Ok(TypeExpr::Union { variants: types, span })
        }
    }

    pub(crate) fn parse_intersection_type_expr(&mut self) -> Result<TypeExpr, ()> {
        let mut t = self.parse_base_type_expr()?;
        while self.eat(&Token::Ampersand) {
            let right = self.parse_base_type_expr()?;
            let span = t.span().merge(right.span());
            t = TypeExpr::Intersection { left: Box::new(t), right: Box::new(right), span };
        }
        Ok(t)
    }

    pub(crate) fn parse_base_type_expr(&mut self) -> Result<TypeExpr, ()> {
        let start = self.span();
        if self.eat(&Token::LParen) {
            let mut elements = Vec::new();
            if !self.eat(&Token::RParen) {
                loop {
                    elements.push(self.parse_type_expr()?);
                    if !self.eat(&Token::Comma) { break; }
                }
                self.expect(&Token::RParen)?;
            }
            if elements.is_empty() { return Ok(TypeExpr::Unit { span: start.merge(self.span()) }); }
            return Ok(TypeExpr::Tuple { elements, span: start.merge(self.span()) });
        }
        let name = self.parse_ident_name()?;
        if self.eat(&Token::LBracket) {
            let mut args = Vec::new();
            loop {
                args.push(self.parse_type_expr()?);
                if !self.eat(&Token::Comma) { break; }
            }
            self.expect(&Token::RBracket)?;
            let span = start.merge(self.span());
            if name == "Map" && args.len() == 2 {
                let mut it = args.into_iter();
                Ok(TypeExpr::Map { key: Box::new(it.next().ok_or(())?), value: Box::new(it.next().ok_or(())?), span })
            } else if name == "Set" && args.len() == 1 {
                Ok(TypeExpr::Set { element: Box::new(args.into_iter().next().ok_or(())?), span })
            } else {
                Ok(TypeExpr::Generic { name, args, span })
            }
        } else {
            Ok(TypeExpr::Named { name, span: start.merge(self.span()) })
        }
    }
}
