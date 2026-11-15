" Vim syntax for Grant (Foundation 1)
if exists("b:current_syntax")
  finish
endif

syn keyword grantStatement from kind rule never each world
syn keyword grantLogic     if where and or not in the
syn keyword grantFunction  count sum min max div mod id
syn keyword grantHowMany   one maybe some many
syn match   grantNumber    "\<\d\+\(\.\d\+\)\?\>"
syn region  grantText      start=+"+ skip=+\\.+ end=+"+
syn match   grantComment   "--.*$"
syn match   grantAsk       "^\s*?"
syn match   grantChange    "^\s*[+-]\ze\s"
syn match   grantRelation  "\<\h\w*\ze\s*:"

hi def link grantStatement Keyword
hi def link grantLogic     Conditional
hi def link grantFunction  Function
hi def link grantHowMany   Type
hi def link grantNumber    Number
hi def link grantText      String
hi def link grantComment   Comment
hi def link grantAsk       Special
hi def link grantChange    Operator
hi def link grantRelation  Identifier

setlocal commentstring=--\ %s
let b:current_syntax = "grant"
