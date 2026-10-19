; Tag queries for the gleam lang pack (feature 002, chunk:m5-lang-pack).
;
; The registry's contract (crates/magistr-workspace/src/lang_registry.rs):
;   @name              the symbol's name node
;   @definition.<kind> the enclosing definition node — kind maps onto the
;                      indexer's static vocabulary (function, type, const, …)
;   @import            an import target node
;
; Deliberately definition-focused: reference captures (call sites, field
; accesses) belong to navigation tooling, not the Tier-1 symbol index.

(function
  name: (identifier) @name) @definition.function

(external_function
  name: (identifier) @name) @definition.function

(type_definition
  (type_name
    name: (type_identifier) @name)) @definition.type

(type_alias
  (type_name
    name: (type_identifier) @name)) @definition.type

(external_type
  (type_name
    name: (type_identifier) @name)) @definition.type

(constant
  name: (identifier) @name) @definition.constant

(import
  module: (module) @import)
