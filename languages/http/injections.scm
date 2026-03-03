((json_body) @content
  (#set! "language" "json"))

((xml_body) @content
  (#set! "language" "xml"))

((graphql_data) @content
  (#set! "language" "graphql"))
