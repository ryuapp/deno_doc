use super::SymbolContentCtx;
use crate::function::FunctionDef;
use crate::html::parameters::render_params;
use crate::html::render_context::RenderContext;
use crate::html::types::render_type_def;
use crate::html::types::render_type_def_colon;
use crate::html::types::type_params_summary;
use crate::html::util::*;
use crate::html::DocNodeWithContext;
use crate::js_doc::JsDocTag;
use crate::params::ParamPatternDef;
use serde::Serialize;
use std::collections::HashSet;

fn render_css_for_fn(overload_id: &str, deprecated: bool) -> String {
  let (bg_color, border_color) = if deprecated {
    ("#D256460C", "#DC2626")
  } else {
    (
      "var(--ddoc-selection-selected-bg)",
      "var(--ddoc-selection-selected-border-color)",
    )
  };

  format!(
    r#"
#{overload_id} {{
  display: none;
}}
#{overload_id}:checked ~ *:last-child > :not(#{overload_id}_div) {{
  display: none;
}}
#{overload_id}:checked ~ div:first-of-type > label[for='{overload_id}'] {{
  background-color: {bg_color};
  border: solid var(--ddoc-selection-border-width) {border_color};
  cursor: unset;
  padding: var(--ddoc-selection-padding); /* 1px less to counter the increased border */
}}
"#
  )
}

#[derive(Debug, Serialize, Clone)]
struct OverloadRenderCtx {
  function_id: String,
  overload_id: String,
  additional_css: String,
  html_attrs: String,
  name: String,
  deprecated: Option<String>,
  summary: String,
  summary_doc: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FunctionCtx {
  overloads_ctx: Vec<OverloadRenderCtx>,
  functions: Vec<SymbolContentCtx>,
}

impl FunctionCtx {
  pub const TEMPLATE: &'static str = "function";

  pub(crate) fn new(
    ctx: &RenderContext,
    doc_nodes: Vec<&DocNodeWithContext>,
  ) -> Self {
    let mut overloads_ctx = Vec::with_capacity(doc_nodes.len());
    let mut functions_content = Vec::with_capacity(doc_nodes.len());

    for (i, doc_node) in doc_nodes.into_iter().enumerate() {
      let function_def = doc_node.function_def.as_ref().unwrap();

      if function_def.has_body && i != 0 {
        continue;
      }

      let deprecated = doc_node.js_doc.tags.iter().find_map(|tag| {
        if let JsDocTag::Deprecated { doc } = tag {
          Some(
            doc
              .as_ref()
              .map(|doc| crate::html::jsdoc::render_markdown_summary(ctx, doc))
              .unwrap_or_default(),
          )
        } else {
          None
        }
      });

      let overload_id =
        name_to_id("function", &format!("{}_{i}", doc_node.get_name()));
      let id = name_to_id("function", doc_node.get_name());
      let css = render_css_for_fn(&overload_id, deprecated.is_some());

      let summary_doc = if !(function_def.has_body && i == 0) {
        crate::html::jsdoc::jsdoc_body_to_html(ctx, &doc_node.js_doc, true)
      } else {
        None
      };

      let html_attrs = (i == 0)
        .then_some("checked")
        .unwrap_or_default()
        .to_string();

      overloads_ctx.push(OverloadRenderCtx {
        function_id: id.to_string(),
        overload_id: overload_id.to_string(),
        additional_css: css,
        html_attrs,
        name: doc_node.get_name().to_string(),
        deprecated,
        summary: render_function_summary(function_def, ctx),
        summary_doc,
      });

      functions_content.push(render_single_function(
        ctx,
        doc_node,
        &overload_id,
      ));
    }

    FunctionCtx {
      overloads_ctx,
      functions: functions_content,
    }
  }
}

pub(crate) fn render_function_summary(
  function_def: &FunctionDef,
  render_ctx: &RenderContext,
) -> String {
  let return_type = function_def
    .return_type
    .as_ref()
    .map(|ts_type| render_type_def_colon(render_ctx, ts_type))
    .unwrap_or_default();

  format!(
    "{}({}){return_type}",
    type_params_summary(render_ctx, &function_def.type_params),
    render_params(render_ctx, &function_def.params)
  )
}

fn render_single_function(
  ctx: &RenderContext,
  doc_node: &DocNodeWithContext,
  overload_id: &str,
) -> SymbolContentCtx {
  let function_def = doc_node.function_def.as_ref().unwrap();

  let current_type_params = function_def
    .type_params
    .iter()
    .map(|def| def.name.as_str())
    .collect::<HashSet<&str>>();
  let ctx = &ctx.with_current_type_params(current_type_params);

  let param_docs =
    doc_node
      .js_doc
      .tags
      .iter()
      .filter_map(|tag| {
        if let JsDocTag::Param {
          name,
          doc,
          optional,
          default,
          ..
        } = tag
        {
          Some((name.as_str(), (doc, *optional, default)))
        } else {
          None
        }
      })
      .collect::<std::collections::HashMap<
        &str,
        (&Option<String>, bool, &Option<String>),
      >>();

  let params = function_def
    .params
    .iter()
    .enumerate()
    .map(|(i, param)| {
      let (name, str_name) = crate::html::parameters::param_name(param, i);
      let id = name_to_id(overload_id, &format!("parameters_{str_name}"));

      let (mut default, optional) = if let Some((_doc, optional, default)) =
        param_docs.get(name.as_str())
      {
        ((**default).to_owned(), *optional)
      } else {
        (None, false)
      };

      let ts_type =
        if let ParamPatternDef::Assign { left, right } = &param.pattern {
          default = default.or(Some(right.to_string()));
          left.ts_type.as_ref()
        } else {
          param.ts_type.as_ref()
        };

      let mut ts_type = ts_type
        .map(|ts_type| render_type_def_colon(ctx, ts_type))
        .unwrap_or_default();

      if let Some(default) = &default {
        ts_type = format!(r#"{ts_type}<span><span class="font-normal"> = </span>{default}</span>"#);
      }

      let tags = if matches!(
        param.pattern,
        ParamPatternDef::Array { optional, .. }
          | ParamPatternDef::Identifier { optional, .. }
          | ParamPatternDef::Object { optional, .. }
        if optional
      ) || default.is_some()
        || optional
      {
        HashSet::from([Tag::Optional])
      } else {
        HashSet::new()
      };

      DocEntryCtx::new(
        ctx,
        &id,
        &name,
        None,
        &ts_type,
        tags,
        param_docs
          .get(name.as_str())
          .and_then(|(doc, _, _)| doc.as_deref()),
        &doc_node.location,
      )
    })
    .collect::<Vec<DocEntryCtx>>();

  let mut sections = vec![];

  let docs =
    crate::html::jsdoc::jsdoc_body_to_html(ctx, &doc_node.js_doc, false);
  let examples = crate::html::jsdoc::jsdoc_examples(ctx, &doc_node.js_doc);

  if let Some(examples) = examples {
    sections.push(examples);
  }

  if let Some(type_params) = crate::html::types::render_type_params(
    ctx,
    &doc_node.js_doc,
    &function_def.type_params,
    &doc_node.location,
  ) {
    sections.push(type_params);
  }

  if !params.is_empty() {
    sections.push(SectionCtx {
      title: "Parameters".to_string(),
      content: SectionContentCtx::DocEntry(params),
    });
  }

  sections.push(SectionCtx {
    title: "Return Type".to_string(),
    content: SectionContentCtx::DocEntry(
      render_function_return_type(ctx, function_def, doc_node, overload_id)
        .map_or_else(Default::default, |doc_entry| vec![doc_entry]),
    ),
  });

  SymbolContentCtx {
    id: format!("{overload_id}_div"),
    sections,
    docs,
  }
}

fn render_function_return_type(
  render_ctx: &RenderContext,
  def: &FunctionDef,
  doc_node: &DocNodeWithContext,
  overload_id: &str,
) -> Option<DocEntryCtx> {
  let return_type = def.return_type.as_ref()?;

  let id = name_to_id(overload_id, "return");

  let return_type_doc = doc_node.js_doc.tags.iter().find_map(|tag| {
    if let JsDocTag::Return { doc, .. } = tag {
      doc.as_deref()
    } else {
      None
    }
  });

  Some(DocEntryCtx::new(
    render_ctx,
    &id,
    "",
    None,
    &render_type_def(render_ctx, return_type),
    HashSet::new(),
    return_type_doc,
    &doc_node.location,
  ))
}
