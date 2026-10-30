# Horizon com os patches do NTM

**Commit base:** upstream `9e49037875b91efe07a2d5e4714a113d052df17a` (main de 2026-10-29).
**Worktree de build:** `~/Workspace/horizon-patched`, branch `ntm-patched`, com um commit por patch sobre a base
(`git log 9e490378..ntm-patched`).

## Ordem de aplicação

Os patches mexem em arquivos distintos, então qualquer ordem funciona. A ordem dos commits é esta:

| # | Patch | Arquivos | O que faz |
|---|---|---|---|
| 1 | `creative-plus.diff` | 7 em `crates/horizon-client` (+385/−12, com o teste) | O painel do Recipes ("JEI") no inventário criativo, com modo cheat e R/U; as abas criativas de content mods (`data/<ns>/creative_tab/*.json`), em páginas, e os itens delas na busca criativa |
| 2 | `recipes-tick.diff` | `mods/official/recipes/src/lib.rs` | O Recipes carrega as tabelas em etapas, no `tick` e não no `frame`, compara itens por índice e, quando a busca não acha nada, mostra uma barreira em vez de sumir com o painel. Sem isso ele morre com o conteúdo completo do NTM |
| 3 | `mod-item-tags.diff` (da fundação, versão final com `rustfmt`) | `crates/horizon-client/src/mod_content.rs` | As tags de content mods passam a valer em comandos (`/clear @s #ntm:rad/bit_8`) |
| 4 | `content-read-once.diff` (formato `git format-patch`; aplica com `git apply` ou `git am`) | `crates/horizon-mods/src/content.rs` | `Content::check` lê as declarações block/item por um único leitor do zip, em vez de reabrir o `.hmod` (e reler o índice de 12.086 entradas) a cada arquivo. Com o ntm.hmod, `Content::read` caía de 109 s para 0,11 s; ele roda 2× por início (carregador e `start_mods` na thread de render), por isso a janela ficava preta/sem responder por ~4 min |
| 5 | `ntm-visual.diff` (branch `ntm-visual` em `~/Workspace/horizon-visual`, 42 commits sobre o #4, **já em `ntm-patched`** por fast-forward: 0fd6da96d) | 48 arquivos em `horizon-world`, `horizon-mods`, `horizon-mod-sdk`, `horizon-client-render`, `horizon-client`, `cargo-horizon-mod`, docs | O visual fiel do NTM ([../docs/visual-fiel.md](../docs/visual-fiel.md)): máquinas de content mods com slots de verdade no motor (block entity, menu, sync, save, drop), telas sobre a textura de GUI do mod (barras, luzes, tooltips, botões), modelos OBJ com animações no formato do GeckoLib, multiblocos (dummies, quebra junto, contorno único), a capability de servidor `machines` (com o dano do item), `max_damage` nos itens, `reject` nos slots, modelos desenhados de malhas na GPU (instâncias por osso) texturas de tela carregadas ao abrir e o lado de quem colocou guardado e sincronizado para máquinas sem `facing` (as bombas). Para a fundição: líquidos nos modelos e no `horizon_liquid` (metal tingido pelos valores `synced` do block entity); blocos com modelo ou líquido abertos (sem esconder as faces dos vizinhos, a luz passa); nenhum menu para máquinas sem slots; título da tela colorido (`title_color`). Para as bombas: `max` por slot (uma peça por slot). Para a bigorna: `shift` nos botões e os botões das telas de máquina no `horizon-ctl`. Para a etapa d.2: `from` nas barras e os ponteiros (`gauges`); o repouso das animações no primeiro quadro. Para o bedrock: `custom` (o `custom_data` do item, em SNBT) no `item-stack` da API `machines`, api level 10. Partes de modelo por estado do bloco (`show`, `glow`). Líquido sem tinta (`tint`); multibloco com altura e caixas extras (a escavadora). Modelo que abre com a tela (`open`: a porta da fornalha), com o menu de um dummy guardado no núcleo; o `custom_data` dos itens vai aos clientes (no criativo, o shift-clique o apagava). Várias telas por máquina, trocadas por um botão (`screens`, `slots` por tela); partes do modelo mostradas por valor sincronizado (`show` por `value`); a referência dos arquivos em `docs/api/machine-files.md`. Animações posadas por valor sincronizado (`pose`), e esses valores vão aos clientes só como dado do block entity. Os mods do NTM para ele saem do `tools/build-visual.sh` (server mod com `machines`) |

```sh
git clone https://github.com/horizon-rust/horizon.git horizon-patched && cd horizon-patched
git checkout -b ntm-patched 9e49037875b91efe07a2d5e4714a113d052df17a
for p in creative-plus recipes-tick mod-item-tags content-read-once ntm-visual; do git apply ~/Workspace/ntm-port/horizon-patches/$p.diff; done
agent-run cargo build --release -j 6 --features horizon-client/mods -p horizon-client -p cargo-horizon-mod
HORIZON_VISUAL=$PWD ~/Workspace/ntm-port/tools/build-visual.sh   # os 3 .hmod do NTM em ~/Workspace/ntm-port/target/visual-build/dist
(cd mods/official/recipes && ../../../target/release/cargo-horizon-mod horizon-mod build --release)   # target/horizon-mod/recipes.hmod
```

A instalação do usuário (`~/Games/horizon`, commit `56f968e11`) é anterior à mudança do painel para
`native_panel.rs`, e o `creative-plus` **não aplica** nela. Por isso a instalação nova fica **ao lado**: veja
[Migração](#migração).

## 1. creative-plus

- **Painel no criativo.**
  - O painel de um mod visualizador (o Recipes WASM, ou um mod nativo com `native_panel_items`) aparece à direita da
    tela criativa, com busca, páginas, páginas de receita, e R/U sobre itens do painel **e da grade criativa**.
  - **Modo cheat:** é a opção "Click gives items (creative)" do Recipes (Opções > Mods > Recipes, ou o arquivo
    `horizon-mods/.horizon-mod-data/recipes/give` com `true`). O clique esquerdo dá 1 item e o direito, uma pilha.
  - Mudanças:
    - os gates `Screen::Inventory` viraram `Inventory | Creative` (`native_panel.rs`, `mod_input.rs`, `control.rs`);
    - `draw_creative` desenha o painel antes da pilha carregada;
    - `click_creative` passa o clique ao painel primeiro;
    - a busca criativa cede o teclado à caixa do painel quando ela está em foco;
    - `mod_browser_pointed` lê os slots de exibição do criativo.
- **Abas de content mods** (`creative/mod_tabs.rs`):
  - Cada `data/<ns>/creative_tab/<nome>.json` de um content mod vira a aba `<ns>:<nome>`:
    `{title: chave de lang, fallback: inglês, icon, order, items: [ids]}`.
  - O caminho fica fora de `data/<id>/horizon/`, então o `mod_content` já copia o arquivo e nenhum loader mudou. Um
    Horizon sem o patch ignora esses arquivos (`cargo-horizon-mod check` passa no 56f968e11 e no 9e490378).
  - Ficam 10 abas por página, depois das abas vanilla, com botões `<` `>` e "2 / 2"; busca e inventário aparecem em
    todas as páginas.
  - Os itens entram na busca criativa (por nome traduzido e por id; tags `#` ainda não).
  - Gerador: `tools/creative-tabs/generate.py`, já chamado pelo `tools/content-gen/generate.py` no fim.

## 2. recipes-tick: a correção do fuel do Recipes

O Recipes oficial carrega todos os itens e receitas num único `frame`, que tem 10M de *fuel*. Com o ntm.hmod final
(1.881 ids → 3.539 itens no jogo, 806 receitas do NTM) isso gastou **13,1M**, e o mod foi interrompido
("took too long") ao entrar no mundo. Medido com instrumentação temporária no runtime:

| Opção | O que muda | Pico medido | Folga | Contra |
|---|---|---|---|---|
| A. `FUEL_FRAME` 10M → 400M no motor | 1 linha em `horizon-mods` | 13,1M | 30× | Afrouxa **todos** os mods: um laço trava até ~0,3 s de quadro antes de parar |
| B. Orçamento maior só na carga | motor | — | — | Não resolve: busca e R/U também passaram de 10M (13,2M) |
| C. Recipes em etapas no `frame` (10M) | só o mod | 8,7M (`recipes::all()`) | 1,15× | Funciona hoje, mas quebra com o próximo lote de conteúdo |
| **D. Recipes em etapas no `tick` (50M)**, comparação por índice e nomes pré-processados | só o mod (`recipes-tick.diff`) | **8,7M** | **5,7×** | Clique e tecla respondem no próximo tick (≤ 50 ms) |

Escolhida: **D**. O sandbox fica intacto, nenhum quadro paga pelo mod (ele não roda mais no `frame`), a carga é
dividida em 4 ticks (itens, nomes, receitas, lista) e a busca/R/U ficou mais barata: comparar ids de string virou
comparar índices. Custo: o `recipes.hmod` passa a ser um fork do oficial. Se o launcher atualizar o Recipes pelo
registro, ele volta ao oficial (e quebra com o NTM completo). O caminho definitivo é o Recipes nativo que o upstream
está migrando, ou levar o D ao upstream.

## Verificação

**Build e testes** (`~/Workspace/horizon-patched`, os 3 commits, rodados com `agent-run`):

| Verificação | Resultado |
|---|---|
| `cargo build --release --features horizon-client/mods -p horizon-client -p cargo-horizon-mod` | ok, sem warnings (6m43s do zero com `-j 6` na fatia; 4m32s ao refazer o `horizon-client`) |
| `cargo test --release -p horizon-client --features mods --lib --bins` | **lib 628 ok, binário 141 ok, horizon-ctl 18 ok**, 0 falhas (os `#[ignore]` lentos de content root não rodaram) |
| Teste novo `creative::mod_tabs::tests::content_mod_tabs_load_on_a_page_of_their_own` | ok. Ele achou um bug na primeira versão (uma segunda carga da mesma raiz zerava a lista de abas), já corrigido |
| `cargo clippy -p horizon-client -p recipes --features mods --lib --bins --tests -- -D warnings` | limpo |
| `cargo fmt --all --check` | limpo |
| `tools/pr_ready.py` | não rodado inteiro: os harnesses vanilla precisam do `.local/` de paridade (jars e oracles), que esta máquina não tem |

**Aceitação em jogo** (release, Xvfb `:11`, mudo, `agent-run --user sacer`, mods **instalados** em `horizon-mods/`
como o usuário faz). Mods: `ntm.hmod` `3ddb7a5a…` (1.881 ids, 8 abas), os `ntmlogic-*` de `target/rad-dist` e o
`recipes.hmod` do patch. Roteiro e evidências em `screenshots/aceitacao/` (`accept.py`, `evidence.json`):

| Passo | Resultado | Captura |
|---|---|---|
| Abas do NTM e páginas | "creative-plus: 8 content mod tabs"; página 2/2 com as 8 abas; aba Bombas | `a02`, `a03` |
| Busca criativa | "uranium" acha os itens do NTM | `a04` |
| Painel do Recipes no criativo | 46 páginas de itens à direita; a busca "uranium" filtra; com "zzzz" aparece uma barreira e a caixa continua (antes, o painel sumia) | `a01`, `a05`, `a05a` |
| Modo cheat | clique esquerdo e direito num item do painel: o inventário passou de vazio para `ntm:block_u233` ×65 | `a06` |
| R e U | R num item do painel e U num item da grade criativa abrem receitas e usos | `a07`, `a08` |
| Tags de mod em comandos | `clear @s #ntm:rad/bit_8 0` → "Found 2 matching item(s)"; `bit_0` → "No items were found"; `execute if block … #minecraft:needs_iron_tool` com `ntm:ore_uranium` passou | `evidence.json` |
| Radiação | 2 lingotes de urânio em sobrevivência: Geiger em **0,700 RAD/s**, dose de 0,5 a 29,2 RAD em 40 s | `a09` |
| Máquina | queimador de madeira: "Pôr combustível da mão" consumiu 1 carvão (4 → 3); ligado, 18,6k HE em ~10 s | `a10` |
| Sair e voltar | mesmo mundo; dose de 29,91 RAD mantida; inventário intacto; o queimador seguiu gerando (88,6k HE); painel e abas presentes | `b01`–`b03` |
| Recipes sob carga | nenhum "stopped" no log, na carga e no uso; pico de 8,7M de 50M de fuel | log |

## Migração

A instalação nova fica **ao lado** da atual. O `~/Games/horizon` só é lido, nunca escrito.

```sh
bash ~/Workspace/ntm-port/horizon-patches/install-side-by-side.sh          # simulação: mostra o que faria
bash ~/Workspace/ntm-port/horizon-patches/install-side-by-side.sh --apply  # instala (como sacer, não root)
```

| O quê | Onde |
|---|---|
| Jogo novo: `bin/horizon-client` e `bin/horizon-ctl`, do build release do `horizon-patched` | `~/Games/horizon-ntm/` |
| Opções, `horizon-*.txt`, `saves/` e as configurações dos mods (`.horizon-mod-data`) | **copiados** (reflink no btrfs: não ocupam espaço até mudarem) |
| Mods: `ntm.hmod` e `ntmlogic-*.hmod` de `ntm-port/target/rad-dist` + o `recipes.hmod` do patch | `~/Games/horizon-ntm/horizon-mods/` |
| Recursos preparados | os de `~/Games/horizon/.local/horizon-client/26.3`, por `--resources`: sem cópia de 1,3 GB e sem rodar o horizon-prepare |
| Atalho e menu | `~/.local/bin/horizon-ntm` e `~/.local/share/applications/horizon-ntm.desktop` ("Horizon (NTM)") |
| Para voltar | `horizon-ntm-uninstall`: **move** `~/Games/horizon-ntm` para `~/Games/horizon-ntm.removed-<data>` (os mundos jogados ali ficam guardados) e apaga o atalho e o menu. O "Horizon" antigo continua igual |

Os mundos são cópias: o que se joga no "Horizon (NTM)" não aparece no "Horizon" antigo, e vice-versa. Um mundo
aberto no build novo pode não abrir no antigo (o conteúdo do NTM é maior), e por isso a cópia protege os originais.

## Manutenção

- **Preso ao commit `9e490378`.** Para atualizar o Horizon: `git fetch` no `horizon-patched`, depois
  `git rebase <novo commit> ntm-patched`, um release e a aceitação. Os pontos sensíveis são `game.rs`,
  `native_panel.rs` e `creative.rs`, que mudam com frequência no upstream. Estimativa: de 15 min a 1 h por
  atualização, mais um build release (~7 min aqui, com `-j 6` na fatia baixa) e uma rodada de aceitação.
- **Sem sandbox:** os patches são código do motor (o Recipes continua no sandbox WASM).
- **Upstream:** o creative-plus daria uma API 3 de hooks (painel no criativo, `creative_tab` de content mods); o
  recipes-tick e o mod-item-tags são correções genéricas, e os três valem uma PR upstream para acabar com o fork.

## creative-plus-mixins/: variante como mod nativo (experimental, só as abas)

Um bundle do horizon-mixins com as abas e os botões de página. O plano passa no `9e490378`, mas o bundle **não foi
compilado nem testado**. O painel no criativo não sai como mixin: os gates têm closures, e o horizon-mixins 0.5
recusa OVERWRITE e RETURN nessas funções. Fica como referência para uma API 3 de hooks no upstream.
