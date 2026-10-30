# C2: block entities das bombas nucleares

Os sete blocos `nuke_boy`, `nuke_gadget`, `nuke_man`, `nuke_mike`, `nuke_tsar`, `nuke_n2` e `nuke_fstbmb` usam agora o template `minecraft:beehive`, com `facing` horizontal e `honey_level` de 0 a 5. O contrato foi combinado com a frente de bombas nativas: o block entity guarda armado, temporizador e slots; redstone será lida por helper nativo. `logic.powered` é `null` nesses sete registros, pois o beehive não fornece o `lit` da lâmpada anterior.

A mudança atende à exigência do hook de block entities do Horizon atual: o bloco template precisa oferecer um block entity para o ciclo de criação/remoção funcionar. A orientação chama-se `facing`, com north/east/south/west. O registro Rust e a leitura do sinal de redstone são responsabilidade da frente nativa.

Os IDs, modelos, receitas e loot foram preservados. As quatro cargas simples (`tnt_ntm`, `dynamite`, `semtex`, `c4`) continuam com o template `redstone_lamp` e os estados `lit=false/true`. A instalação antiga não recebe esse pacote; o alvo é a instalação nativa nova.

Solução reaproveitada: `bomb_catalog.py` e a emissão existente de estados no content-gen; a prova é mais um perfil do verificador de conteúdo, usando `data get block`, as ações reais `break` e `place` do control socket, com o item na mão num mundo descartável.

A prova no ntm-next com o bundle NTM/Recipes passou em 183 verificações, num mundo próprio em Xvfb e sem áudio: os sete tipos mostraram block entity com o estado `Bomb` e seus slots, nos níveis 0 e 5; todos foram quebrados pela ação `break`, recolocados pela ação `place` e novamente consultados com `data get block`. Detector de rejeição e panic limpo. A prova não arma nem detona bombas.

Passaram 172 testes Python em cada seed 1/42 e os dois validadores de conteúdo/receitas. Evidências locais em `content-gen-evidence/migration-c2/`; pacote persistente `dist/ntm-content-c2.hmod` (6.528.147 bytes, SHA-256 `97c4c7df86c7730d5963508a62f33f814c6aceb05213459efabdbc50fce9eb85`).

O check do fork e o check do upstream passaram no mesmo pacote. A pasta de mundo usada na prova foi removida após guardar os logs; nenhuma instalação real recebeu o pacote.
