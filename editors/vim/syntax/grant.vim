" Vim syntax for Grant (Foundation 1)
"
" Grant reads names by where they stand, so a name alone doesn't say what it is.
" This file reads the world, and the worlds it builds on or holds, to learn its
" kinds, relations and objects, and colours each name by what it is.
if exists("b:current_syntax")
  finish
endif

syn match   grantDelimiter "[][()|:,.]"
syn match   grantOperator  "[-+*/=<>]\|!=\|<=\|>="
syn keyword grantStatement from kind rule never each
syn keyword grantLogic     if where and or not in the
syn keyword grantOperator  div mod
syn keyword grantFunction  count sum min max
syn keyword grantHowMany   one maybe some many
syn keyword grantPrimitive number text id world numbers
syn match   grantNumber    "\<\d\+\%(\.\d\+\)\?\>"
syn region  grantText      start=+"+ skip=+\\.+ end=+"+ contains=grantEscape
syn match   grantEscape    +\\[n"\\]+ contained
syn match   grantComment   "--.*$" contains=@Spell
syn match   grantAsk       "^\s*?"
syn match   grantRelation  "\%(\<in\s\+\)\@<!\<\h\w*\ze\s*:"
syn match   grantRelation  "\.\@1<=\h\w*"
" What a bracket entry holds, after its `:`. Objects are left plain there, so the relations
" stand out. The `:` after a rule's name or after `in w` starts no entry.
syn region  grantValues    matchgroup=grantDelimiter oneline contains=ALLBUT,grantObjectName
      \ start="\%(^\s*\%(-\s*\)\?\%(rule\|never\)\s\+\h\w*\s*\)\@<!\%(\<in\s\+\h\w*\s*\)\@<!:" end="\ze[|\]]" end="$"
syn match   grantKind      "\%(^\s*\%(-\s*\)\?kind\s\+\)\@<=\h\w*"

" Reads the lines of a world, then the worlds it names with `from` or `world`.
function! s:Read(lines, dir, seen, found) abort
  for l:line in a:lines
    let l:line = substitute(l:line, '--.*$', '', '')
    let l:worlds = split(matchstr(l:line, '^from\s\+\zs.*'), '\s*,\s*')
    let l:held = matchstr(l:line, '^world\s\+\zs[A-Za-z0-9_./]\+\ze\s*\%(\[.*\)\?$')
    if l:held !=# ''
      let a:found.objects[fnamemodify(l:held, ':t')] = 1
      call add(l:worlds, l:held)
    endif
    for l:world in l:worlds
      let l:file = resolve(simplify(a:dir . '/' . trim(l:world) . '.grant'))
      if filereadable(l:file) && !has_key(a:seen, l:file)
        let a:seen[l:file] = 1
        call s:Read(readfile(l:file), fnamemodify(l:file, ':h'), a:seen, a:found)
      endif
    endfor
    let l:kind = matchstr(l:line, '^\%(-\s*\)\?kind\s\+\zs\h\w*')
    if l:kind !=# ''
      let a:found.kinds[l:kind] = 1
    endif
    " Every `name:` is a relation, but for a rule's name and the world after `in`.
    let l:entries = substitute(l:line, '^\s*\%(rule\|never\)\s\+\h\w*\s*:', '', '')
    let [l:relation, l:from, l:to] = matchstrpos(l:entries, '\%(\<in\s\+\)\@<!\<\h\w*\ze\s*:')
    while l:from >= 0
      let a:found.relations[l:relation] = 1
      let [l:relation, l:from, l:to] = matchstrpos(l:entries, '\%(\<in\s\+\)\@<!\<\h\w*\ze\s*:', l:to)
    endwhile
    call add(a:found.lines, l:line)
  endfor
endfunction

function! s:Colour(group, names) abort
  execute 'silent! syn clear ' . a:group
  if !empty(a:names)
    execute 'syn match ' . a:group . ' "\<\%(' . join(keys(a:names), '\|') . '\)\>"'
  endif
endfunction

" Learns the names of this world and colours them.
function! s:Learn() abort
  let l:found = {'kinds': {}, 'relations': {}, 'objects': {}, 'lines': []}
  call s:Read(getline(1, '$'), expand('%:p:h'), {resolve(expand('%:p')): 1}, l:found)
  " An object is stated as `kind handle`, alone on its line or before brackets.
  for l:line in l:found.lines
    let l:stated = matchlist(l:line, '^\(\h\w*\)\s\+\(\h\w*\)\s*\%(\[.*\)\?$')
    if !empty(l:stated) && has_key(l:found.kinds, l:stated[1])
      let l:found.objects[l:stated[2]] = 1
    endif
  endfor
  call s:Colour('grantObjectName', l:found.objects)
  call s:Colour('grantRelationName', l:found.relations)
  call s:Colour('grantKindName', l:found.kinds)
  " A rule's name is its own, even when an object shares it.
  silent! syn clear grantRuleName
  syn match grantRuleName "\%(^\s*\%(-\s*\)\?\%(rule\|never\)\s\+\)\@<=\h\w*"
endfunction

call s:Learn()
augroup grant_syntax
  autocmd! * <buffer>
  autocmd BufWritePost,InsertLeave <buffer> call s:Learn()
augroup END

if has('nvim')
  hi def link grantRelation   @variable.builtin
  hi def link grantObjectName @lsp.type.enumMember
  hi def link grantEscape     @string.special
endif
hi def link grantStatement    Keyword
hi def link grantLogic        Keyword
hi def link grantOperator     Operator
hi def link grantFunction     Function
hi def link grantRuleName     Function
hi def link grantHowMany      StorageClass
hi def link grantPrimitive    Type
hi def link grantKind         Type
hi def link grantKindName     Type
hi def link grantNumber       Number
hi def link grantText         String
hi def link grantEscape       SpecialChar
hi def link grantComment      Comment
hi def link grantAsk          Special
hi def link grantDelimiter    Delimiter
hi def link grantRelation     Identifier
hi def link grantRelationName grantRelation
hi def link grantObjectName   Constant

setlocal commentstring=--\ %s
let b:current_syntax = "grant"
