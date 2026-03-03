; Methods
(method) @function.method

; URLs
(request
  url: (_) @string.special.url)

; HTTP version
(http_version) @constant

; Headers
(header
  name: (_) @constant)
(header
  ":" @punctuation.delimiter)

; Variables
(variable_declaration
  name: (identifier) @variable)
(variable_declaration
  "=" @operator)

; Variable interpolation braces
[
  "{{"
  "}}"
] @punctuation.bracket

; Comment annotations (@name, @no-log, etc.)
(comment
  "@" @keyword
  name: (_) @keyword)
(comment
  "=" @operator)

; Response status
(status_code) @number
(status_text) @string

; External body file path
(external_body
  path: (_) @string.special.path)

; Comments and separators
[
  (comment)
  (request_separator)
] @comment
