# Horizon atual e bombas otimizadas

Decisão do usuário em 01/11/2026: trabalhar com o Horizon oficial mais recente,
adaptando o NTM quando as interfaces mudarem. O visual aprimorado foi arquivado;
suas configurações antigas são migradas para o otimizado sem apagar as demais opções.

## Base de desenvolvimento

- Projeto: `/mnt/ssd120/sacer/horizon-ntm`.
- Motor: `engine/horizon-ntm-next`, branch `ntm-current`.
- Upstream incorporado: `a3bf6531e07f6292414eda44773d30ab0a47f9db` (snapshot congelado de 09/11/2026; ver a sexta atualização).
- Motor de integração (pin): `b7db6d9856fd67180cc916b2997fc3a3707d82c5`.
- Pins do motor, mixins e Recipes: `native/ntm/compatibility.json`.
- K3 e energia solar/Stirling foram integrados nesta base; a antiga branch
  `ntm-next` continua preservada. Não usar seus pins antigos para novos desenvolvimentos.

O motor continua independente do NTM. Foram mantidos os modelos OBJ, animações,
multiblocos, tamanhos exatos de contêineres, filtros para menus/funis, dados de menu
com 32 bits e valores de jogador com 64 bits. A sincronização de block entities,
os componentes de itens, sons/partículas e páginas criativas aproveitam o upstream.
Essas extensões locais ainda exigem a revisão exata de integração: o número da API
oficial sozinho não garante compatibilidade.

## Compilar e atualizar

`tools/build-native.sh` encontra o motor relativamente ao projeto, consulta o
`main` oficial e exige que ele seja ancestral da revisão selecionada. Se o upstream
avançou, o script para indicando o novo SHA. Incorporar a atualização na branch
ativa, adaptar os pontos afetados, executar os testes e atualizar os dois pins
(`compatibility.json` e `horizon-mixin.toml`) antes de prosseguir.

O build usa `rsync` para conservar os arquivos intermediários idênticos e evitar
recompilar o motor inteiro quando só o bundle mudou. O alvo de compilação fica
fora da árvore intermediária sincronizada.

```sh
agent-run env QUEUE=cargo CARGO_BUILD_JOBS=2 \
  BUILD_DIR=/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/current-native \
  CARGO_TARGET_DIR=/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/migracao-upstream/target \
  bash tools/build-native.sh
```

O build nativo não regenera `dist/ntm.hmod`. Depois dele, empacotar o conteúdo da
mesma `main` com a ferramenta release recém-compilada, antes da aceitação em jogo:

```sh
agent-run env QUEUE=cargo CARGO_BUILD_JOBS=2 \
  HORIZON_MOD=/mnt/ssd120/sacer/horizon-ntm/.agent-tmp/migracao-upstream/target/release/cargo-horizon-mod \
  bash tools/build-content.sh
```

Registrar os hashes do executável e desse pacote juntos; a presença de máquinas
antigas no pacote não comprova que ele contém os itens adicionados depois.

Os executáveis são escritos em `$CARGO_TARGET_DIR/release`. A aceitação exige
build release; `NATIVE_PROFILE=fast` serve apenas para iteração e não conta como validação. `SKIP_UPSTREAM_CHECK=1` só serve para repetir ou
reproduzir uma revisão já selecionada; sem essa opção, toda execução consulta o
Horizon oficial. O script não instala o jogo e não abre mundos do jogador.

O antigo `tools/test-game/run-visual.sh` é do protótipo WebAssembly; para esta
base, usar o executável produzido por `build-native.sh` com `dist/ntm.hmod`.
Nos testes, configurar `HORIZON_HOME`, `HORIZON_LAUNCHER_HOME`, `XDG_DATA_HOME` e
`XDG_CONFIG_HOME` em uma pasta descartável, com `HORIZON_NO_AUDIO=1`.
Executar o cliente tendo essa pasta descartável como diretório de trabalho:
caminhos relativos de mundos e cache também dependem dele.
Os roteiros com XTest precisam de uma janela visível dentro do Xvfb isolado;
nesses casos, omitir `--offscreen`, que oculta a janela e impede o foco do teclado.

Resultados da migração inicial estão no artifact `horizon-atualizacao`;
o fechamento seguinte está em `energia-solar-stirling-integracao`, com as provas
de K3, geração solar/Stirling e reabertura de mundo no snapshot acima.

## Auditoria das extensões — 02/11/2026, segunda atualização

O intervalo auditado foi `d928d1cd6..e8b55fbe5`. A primeira integração incorporou
`0e24521cd`; as consultas seguintes detectaram e incorporaram `693aa8482`,
`fbb17c980`, `ba832babb` e `e8b55fbe5`.
Motor de integração: `c8d86c4b5c2f3673d91445da5d5d906624bf6bed`. A revisão independente dos três conflitos não encontrou
problemas. Dano direto (`generic_kill`) e efeitos de radiação foram exercitados no servidor
integrado, além das GUIs e da bomba otimizada; a rodada desse pin passou (ver a terceira atualização abaixo).

| Extensão / caminho local | Resultado da comparação com o upstream | Decisão |
| --- | --- | --- |
| OBJ, ossos, animações, líquidos e culling (`mod_models`, `gfx/mod_models`, `block_entity_render`) | As mudanças oficiais tratam modelos vanilla, equipamento de mobs e itens soltos; não carregam o formato declarativo OBJ/animações usado pelo NTM. | Mantida. O desenho local foi combinado com a nova ordem oficial de sombras antes dos itens translúcidos. |
| Multiblocos (`horizon-world/mod_multiblocks`, `game/multiblocks`) | Não surgiu declaração de volumes, dummies, núcleo, alcance e quebra conjunta equivalente. | Mantida, incluindo a validação do clique nas células. |
| Tamanho exato de contêiner (`Kind::NativeContainer`, `lan/menu`) | Os menus oficiais seguem templates vanilla; não substituem a contagem arbitrária usada nas máquinas. | Mantida para preservar índices e slots sem arredondamento. |
| Filtros e limites por slot (`mod_slot_filters`, menus e funis) | Nenhum equivalente novo no intervalo de upstream; hooks de clique não garantem a mesma regra em previsão, servidor e automação. | Mantida uma regra compartilhada entre os três caminhos. |
| Dados de menu i32 (`native_hooks`, `lan/containers`) | O upstream ainda recebe palavras i16 no protocolo vanilla. | Mantidos os pares de palavras e a compatibilidade com servidores que só enviam i16. |
| Valores de jogador i64 (`native_players`, `player_values`) | Sem canal oficial equivalente para os medidores do NTM. | Mantido o payload genérico, sem reduzir a precisão do HUD. |
| Jogadores, inventários, dano e efeitos (`native_players`) | Os hooks novos observam eventos; não substituem os helpers em lote por UUID usados pelo NTM. | Mantidos, reaproveitando os comandos e efeitos oficiais internamente. |
| Mundo carregado, lotes estritos e sinal redstone (`native_world_access`) | Helpers oficiais de colunas e edição já existentes não preservam integralmente consulta carregada por dimensão, recusa atômica do lote e sinal recebido. | Mantidos esses contratos, sem duplicar geração de chunks. |
| Assets montados (`native_assets`) | O novo leitor `prepared_files` cuida de arquivos de recursos; não oferece a API restrita a `assets/`, listagem recursiva e aliases montados do bundle. | Mantida a interface local. O HUD adotou o leitor oficial para `.png.mcmeta` na resolução do merge. |
| Interação de bloco (`native_use_block`) | Não surgiu hook com posição clicada + núcleo multibloco + bloqueio da segunda mão. | Mantido para os controles e máquinas locais. |
| Entrada e relayout das telas de blocos (`NativeScreenEvent`, `native_screen_set_layout`) | Hooks oficiais genéricos já existiam; os commits novos não acrescentam um contrato equivalente com bloco, dados da máquina e relayout dinâmico. | Mantida a interface local e a entrega separada de teclas/caracteres. |
| Painéis de receita e busca (`creative_plus`, `native_panel`) | A correção oficial de teclas na busca criativa foi incorporada; não substitui o painel lateral de receitas nem o foco próprio do bundle Recipes. | Mantidos apenas os pontos locais necessários ao painel. |
| Rede de block entities, componentes de itens, som/partículas e páginas criativas | Já seguem os mecanismos oficiais integrados anteriormente. As novas alterações oficiais foram incorporadas. | Continuam oficiais; nenhuma segunda implementação adicionada. |

Nenhum substituto oficial novo cobriu integralmente uma das extensões locais neste intervalo.
Não foi removida uma extensão funcional para aparentar simplificação. A simulação in-frame foi
removida pelo próprio upstream; a resolução aceitou essa remoção e preservou o servidor integrado.

A regressão nuclear revelou um bug preexistente do upstream: o envio de `flash`
omitia a cor ARGB exigida pelo decoder. A integração acrescenta `Flash` ao caminho
oficial de cores e um teste separado de codificação/decodificação branco e verde,
preservando o teste upstream existente. É candidato a PR upstream; nada foi publicado.

Os roteiros antigos baseados em `/ntmbomba` dependem do protótipo WebAssembly.
No cliente nativo atual, a prova usa a bomba Little Boy montada com seus cinco
componentes, sinal de redstone e `saves/<mundo>/data/ntm/bombcfg` em uma cópia
descartável do mundo, com `mode: Otimizado`, `max_radius: 12` e fallout desligado.

## Terceira atualização — 02/11/2026, snapshot `2f35078b3`

Intervalo auditado: `e8b55fbe5..2f35078b3` (30 commits, 25 arquivos). A incorporação foi um
merge sem conflitos: `2ca67d71e`. O upstream já estava em `11440651b` quando o build rodou;
por isso o build usou `SKIP_UPSTREAM_CHECK=1` só para esse SHA congelado. Os avanços
posteriores ficam para a próxima rodada.

O delta traz: três hooks de jogador (`native_player_clone`, `native_player_load_from_file`,
`native_player_save_to_file`), corpos padrão em todos os métodos de `Host` (um
`unimplemented!("Host::nome")`, para dublês de teste), diretório `players/data` criado antes dos
hooks de arquivo, notificações de salvamento/entrada/saída no protocolo de gestão do servidor e
parada de itens, TNT e blocos em queda na borda do mundo.

| Extensão local | Equivalente novo no intervalo | Decisão |
| --- | --- | --- |
| OBJ/animações, multiblocos, tamanho exato de contêiner, filtros de slot, menu i32, valores i64, mundo carregado, assets montados, interação de bloco, telas de blocos, painéis de receita | Nenhum. O intervalo só mexe em jogador (hooks de clone/arquivo), `Host`, borda do mundo e protocolo de gestão. | Mantidas. |
| Jogadores, inventários, dano e efeitos (`native_players`) | Os hooks novos de clone/carregar/salvar observam eventos de ciclo de vida; o NTM não guarda estado próprio por arquivo de jogador e os helpers em lote por UUID continuam sem equivalente. | Mantida. Os hooks ficam disponíveis para persistência futura de dados por jogador. |
| Métodos de `Host` locais (API gerada) | Os corpos padrão que entraram valem também para os métodos locais, sem alterar o NTM. | Nada a trocar. |

Nenhuma extensão local foi substituída: não surgiu equivalente oficial novo.

Verificação do pin `2ca67d71e` (build release, sem testes em fast):

- Build release: passou em 7m20s.
- Aceitação em jogo descartável (Xvfb isolado, cliente release): catálogo K3 (340 verificações,
  94 itens), prensa elétrica, eletrolisador, radiação (efeitos e dano), Stirling e solar, e
  Little Boy otimizada (raio 12, cratera, `generic_kill` pelo dano nativo). O log não contém erro
  de decode do `flash`.
- Reabertura: o mesmo mundo foi encerrado e reaberto; o estado salvo das máquinas conferiu
  idêntico (entidades do roteiro Stirling/solar).
- Motor: 122 testes nativos, 25 de menus, 25 de renderização de mods, 57 de conteúdo
  (incluindo os ignorados), 4 de multiblocos, 2 de valores de jogador, 3 de partículas e
  2 do dublê de `Host`. Todos passaram.
- NTM: 295 testes Rust passaram (2 ignorados); Clippy `-D warnings` no workspace do NTM passou.
- Clippy `-D warnings` nas bibliotecas afetadas: passou na fonte original do motor. Na cópia
  transformada pelos mixins, três `collapsible_if` do código upstream aparecem porque a
  transformação remove os comentários entre os `if`; com esse único lint isentado, passou.
  Dois avisos novos do Rust 1.99 no código do próprio NTM foram corrigidos
  (`screens.rs`: `is_multiple_of`; `glue.rs`: referência desnecessária).
- Os testes nativos rodaram sobre a fonte original do motor, não sobre o código transformado.


## Quarta atualização — 02/11/2026, snapshot `2050c4e0f` — validado

Congelado no início em `2050c4e0f52b74ad5361f061e0a015e8bfc894b2`, 628 commits sobre
`2f35078b3`. Merge no motor: `18eab7ab41e767d9e21dc4079cb2f21dbe116c95`.
O build release usa `SKIP_UPSTREAM_CHECK=1` apenas para este snapshot.
Avanços posteriores serão tratados na próxima rodada.

| Extensão local | Decisão nesta rodada | Comparação com o snapshot oficial |
| --- | --- | --- |
| Replay pós-commit (`native/ntm/mixins/ntm/committed_updates.rs`, módulo do bundle em `horizon-world::level`) | Adicionada na rodada ZIRNOX de 03/11; sem mudar checkout/pin | `Sim/Behaviours::sim` são `pub(crate)` no upstream. Ponte mínima para concluir os updates de uma publicação atômica por meio do `Sim` existente. Reavaliar a cada atualização e remover quando houver entrada pública equivalente. |
| OBJ, ossos, animações, líquidos e culling (`mod_models`) | Mantida | As novas poses de mão, skins e quads de entidade não carregam os modelos declarativos OBJ do NTM. Capacidade do buffer combina modelos locais e contornos oficiais. |
| Multiblocos (`mod_multiblocks`, `game/multiblocks`) | Mantida | Sem contrato oficial para volumes, dummies, núcleo, alcance e quebra conjunta. |
| Contêiner de tamanho exato (`Kind::NativeContainer`, `lan/menu`) | Mantida | Hooks novos de inventário/bigorna não criam templates com número arbitrário de slots. |
| Filtros e limites por slot (`mod_slot_filters`) | Mantida | `menu_slot_rules` permanece um hook de menu; a regra local cobre também previsão e funis/automação. |
| Dados de menu i32 | Mantida | Protocolo vanilla continua i16; o par de palavras preserva os dados completos das máquinas. |
| Valores privados de jogador i64 (`player_values`) | Mantida | Novo sync de attachments aceita dados serializados, mas os envia também aos observadores do jogador, persiste com o codec e não oferece `sendToPlayer`. Não preserva o canal efêmero exclusivo do dono. |
| Jogadores, inventário, dano e efeitos (`native_players`) | Mantida | Hooks novos observam/cancelam eventos; não substituem consultas e ações em lote por UUID. O escoamento após o ServerTickEnd permanece. |
| Visita completa de nível (`Game::visit_level`) | Mantida | `levels.rs` oficial não mudou no intervalo. Entrada normal ainda zera idle e limpa partículas/pickups; a visita restaura tudo inclusive em unwind, sem abrir dimensão. |
| Mundo carregado, lote estrito e redstone (`native_world_access`) | Mantida | Helpers oficiais `edit_blocks`/`set_block` não oferecem o mesmo contrato por dimensão aberta, validação prévia do lote e leitura de comparador/lid_openers do nível completo. |
| Assets montados (`native_assets`) | Mantida | `mod_image` oficial envia RGBA ao atlas e `instance_dir` retorna a pasta; nenhum substitui leitura restrita a assets montados com aliases/listagem. |
| Interação de bloco (`native_use_block`) | Mantida | `right_click_block` não entrega o núcleo do multibloco nem resolve o consumo entre as duas mãos; controles continuam no hook local. |
| Entrada e relayout de telas de blocos (`NativeScreenEvent`) | Mantida | Novos hooks de abrir/inicializar/fechar observam telas por frame; não fornecem o evento local com bloco, dados da máquina e alteração dinâmica dos slots. |
| Painel de receitas e foco na busca (`creative_plus`, `native_panel`) | Mantida; tooltip adotou a função oficial | Painel/foco local ainda necessário. O merge usa `draw_stack_tooltip` oficial, com a exclusão local do segundo tooltip na tela criativa. |
| Flash (`lan_particles`) | Mantida, candidata a PR | O arquivo oficial não mudou. O encoder ainda omite ARGB de Flash sem o conserto local; branco e verde continuam cobertos por roundtrip. |
| Rede de block entities, componentes de itens, som/partículas e páginas criativas | Seguem oficiais | Mantidos os mecanismos já adotados; novas mensagens oficiais de attachment e melhorias upstream incorporadas. |


Verificações concluídas na fonte original do motor (sem transformação pelos mixins):

| Grupo | Resultado |
| --- | --- |
| Nativos (`native_`) | 125 passaram. |
| Menus | 26 passaram. |
| Renderização/mods | 26 passaram. |
| Conteúdo, incluindo testes ignorados | 61 passaram. |
| Multiblocos | 4 passaram. |
| Protocolo i64 | 2 passaram. |
| Partículas | 3 passaram; o roundtrip de flash também pertence ao filtro nativo. |
| `mod_api` e integração tipada | 3 + 2 passaram. |
| NTM | 300 unitários + 26 de integração passaram; 2 doctests ignorados. |
| Clippy estrito | `-D warnings` passou nas 11 bibliotecas selecionadas do motor e no workspace NTM. |
| Cópia transformada | Somente os 3 `collapsible_if` conhecidos em `game.rs`; todos os demais lints passaram ao isentar só esse lint. A fonte original passa sem isenção. |

Build release: Cargo concluiu em **10min26s** (26 injeções, 5 módulos). O conteúdo
foi reempacotado com o `cargo-horizon-mod` desse release: `build-native.sh` sozinho
não o gera. A primeira tentativa usou um pacote anterior sem `hand_drill_desh`;
essa tentativa ficou arquivada e a aceitação em jogo foi reiniciada integralmente
com o pacote da `main` atual.

Upstream mais novo visto na consulta final: `1411020cbb161f608db9171bc1c70aad1ed002a3`.
Não foi incorporado: está reservado para a próxima rodada, conforme a política de congelamento.


A persistência da pilha exige ar positivo antes da captura. O ensaio longo de
operação pode esgotar o barril e o vent; no primeiro ensaio ambos estavam em zero
antes e depois da reabertura, embora a asserção exija `mb > 0`. O driver desta
rodada repõe ar pelo contêiner e pelo botão normais do barril, confirma o vent
positivo e só então captura o estado. `native_pile.py` e suas asserções permanecem
inalterados; não há injeção de estado/NBT nem redução da verificação.


Aceitação final com o pacote reempacotado e o mesmo cliente release:

| Cenário | Resultado |
| --- | --- |
| Catálogo K3 | 340 verificações, 94 itens. |
| Prensa elétrica / eletrolisador | 7 + 4 verificações. |
| Radiação | 2 verificações: efeitos e dano nativo observados. |
| Solar/Stirling | 15 verificações; 21 estados idênticos após salvar, encerrar e reabrir. |
| Little Boy otimizada / flash | 10 verificações: raio 12, cratera, dano `generic_kill`; logs sem erro de decode. |
| `native_dims.py` | 6 verificações na montagem + 4 após reabrir, incluindo dois fornos novos recebendo energia das máquinas retomadas. |
| `native_dims.py idle` / `idle-clean` | 2 + 1 verificações; a dimensão sem jogador fecha e salva com e sem leitor. A fumaça segue apenas como cenário visual. |
| `native_pile.py` | 23 de operação + 25 de reabertura + 31 de destruição, incluindo falha no Overworld com jogador no Nether. Precondição de ar positivo verificada pelo driver (mais 1). |

Os roteiros do repositório foram executados sem alterações; os dez scripts do
usuário continuam byte a byte iguais e fora do commit. Não houve instalação,
push nem uso de mundos reais. Todos os clientes e Xvfb da aceitação foram encerrados.
O OpenGL do Xvfb continua sem TAA; essa rodada valida os fluxos funcionais/UI,
não TAA nesse backend. Os testes são direcionados à integração, não a suíte
completa de paridade vanilla.

Evidências desta rodada: `.agent-tmp/upstream-2050/` (em relação à raiz do projeto),
com `snapshot.json`, `validated-build.json`, `rust-results.json`, `game-summary.json`
e logs. O conjunto aceito tem:

- Motor: `18eab7ab41e767d9e21dc4079cb2f21dbe116c95`.
- SHA-256 do cliente: `e01fe940ff37a2e83f880ef2da74515c35020e60924c640a1a0b2cf7fa022893`.
- SHA-256 de `dist/ntm.hmod`: `d8cd6303742230d743e04220ac1da6af8a3f3bfacbf80a237dafe7b93870e130`.

Pendências: incorporar o upstream posterior ao snapshot em outra rodada; propor
separadamente o conserto genérico do flash ao upstream; os três lints da cópia
transformada continuam sendo uma diferença do transformador. Nenhum ajuste de API
foi necessário no código Rust do NTM: esta adaptação altera apenas os pins e a documentação.

## Quinta atualização — 03/11/2026, snapshot `fc0210381` — validado

Snapshot congelado no início: `fc02103818c212920fd24aa34a2b2f91ba49e56f`, **1.242 commits** após `2050c4e0f`.
Motor de integração: `ec479adb3a66977b339672a7ff8747a7e951068e`; base NTM: `cbcc3006d7661300133f9bbd0cb9a2f0610df1aa`.
`SKIP_UPSTREAM_CHECK=1` foi usado somente para este snapshot; avanços posteriores
ficam para outra rodada. Não houve push nem criação de subagentes.

O merge resolveu dois arquivos: o fechamento criativo conserva o painel local e
a limpeza oficial do foco de controle; os hooks de filtro e contêiner de item
coexistem. Os três hooks locais passaram aos contratos e despachos do gerador
tipado oficial. O atributo recursion_limit do snapshot quebra o mixins fixado:
foi removido e os JSONs aninhados de mobs foram separados sem mudar seus campos.
As novas fixtures upstream recebem filtros/modelos vazios, preservando suas asserções.

As três isenções antigas de collapsible_if da fonte transformada foram eliminadas
com condições equivalentes no motor. Clippy ainda encontrou uma quarta condição
no glue Stirling, ajustada também. A rodada preliminar foi arquivada, e **toda a
aceitação foi reiniciada com o release final**, um único cliente e pacote.

| Extensão | Decisão em fc0210381 | Evidência e limite do mecanismo oficial |
| --- | --- | --- |
| OBJ, ossos, animações, líquidos, culling | Mantida | O novo `horizon-assets/obj.rs`/loader OBJ oficial resolve geometria estática/material para modelos baked. Não lê normais `vn` (o leitor local mantém normais por vértice) nem a composição por ossos, keyframes e líquidos sincronizados. A nova propriedade oficial render_shape seleciona a forma de desenho, mas não cobre por si só o noOcclusion/transparência e a descoberta automática dos arquivos horizon_model/liquid. Substituí-lo aqui degradaria iluminação/animação. |
| Multiblocos | Mantida | Sem declaração oficial de volumes, dummies, núcleo, alcance e quebra conjunta equivalente. |
| Contêiner de tamanho exato | Mantida | Os menus oficiais declarados (`horizon/menu`, já presentes em 2050c4e0f) permitem 1–256 slots. Exigem menu registrado e native_screen; o caminho local também atende contêiner de contagem exata sem registro/tela, e preserva os índices na previsão. Trocar apenas Kind perderia esse contrato; não há substituto novo neste intervalo. |
| Filtros/limites compartilhados por slot | Mantida; despacho trocado pelo oficial | `menu_slot_rules` cobre menus. `Rules::may_pickup` agora protege cliques de hóspedes, mas não é política nativa compartilhada de extração por item/funil. `native_slot_filters` adotou contrato collect e despacho gerado oficial. |
| Menu i32 e valores privados i64 | Mantidas | Os pacotes vanilla de menu ainda são i16. Attachments/EntityRef NBT não equivalem ao canal efêmero preciso exclusivo do dono nem aos pares de palavras existentes. |
| Jogadores/inventário/dano/efeitos em lote por UUID | Mantida | EntityRef ganha velocidade, bounds, dados/tags; não substitui o conjunto de ações/consultas em lote por UUID do NTM. |
| `visit_level` e `native_world_access` | Mantidas | `LevelRef` agora lê estados/propriedades/altura/tempo de níveis abertos, inclusive estacionados. Não oferece visita mutável completa, restauração em unwind, lote validado por inteiro, sinal de comparador/lid_openers e manutenção do idle/partículas. `native_edit_blocks` continua limitado ao nível em tick e sem updates de vizinhos. |
| Assets montados | Mantida | `mod_image`, instance_dir e o novo loader OBJ não substituem leitura restrita a assets montados, aliases e listagem recursiva. |
| Interação no núcleo multibloco | Mantida; despacho trocado pelo oficial | `right_click_block` não resolve núcleo/célula clicada e consumo entre as mãos. `native_use_block` adotou cancel/claimed no gerador oficial. |
| Eventos e relayout de tela de BE | Mantida; despacho trocado pelo oficial | Eventos genéricos de tela não carregam bloco + dados i32 + relayout dos slots. `native_screen_event` adotou cancel/claimed e tipo exportado pelo mod_api oficial. |
| Painel de receitas/foco criativo | Mantida | Novas categorias de receitas declaram tipos/ícones; o próprio contrato diz que ainda não adiciona tabs/displays. Não substitui o painel lateral/foco Recipes. Merge preserva também pad_clear_focus oficial. |
| Flash (`lan_particles.rs`) | Mantida | Mudança oficial neste arquivo apenas troca partículas de blocos por track_explosion. ARGB de Flash continua ausente no encoder oficial. Roundtrip local branco/verde preservado. |
| `[[module]] committed-world-updates` | Mantida | `Sim` e `Behaviours::sim` continuam pub(crate); `set_block` retorna false se o novo estado já foi publicado. Nenhuma entrada pública pós-publicação realiza as callbacks flag 3 depois de reter o conteúdo do BE atomicamente. O módulo delega ao Sim existente. |
| `world_saved` | Oficial mantido, sem duplicação | API 306 existe no snapshot, hook chamado após save_level, save_parked_levels e metadata; wait indica flush. O bundle apenas injeta confirmação/limpeza dos recibos. Não é transação atômica mundo+arquivos do mod. |
| Rede de BEs, componentes de itens, som/partículas, páginas criativas | Oficiais mantidos | Seguem os mecanismos oficiais já integrados. |

Pendências auditadas:

- `stillValid` por identidade do BE: #7973/#7880 acrescenta `BlockEntity::fits` e troca BEs incompatíveis com o bloco. `Slots::valid` ainda só verifica presença na coordenada e `still_valid` acrescenta alcance: não fixa identidade/geração da instância aberta. Correção útil incorporada, contrato completo continua pendente.
- `can_take` condicional por item: #7845 propaga `Rules::may_pickup(slot, stack)` nos cliques de hóspedes e inclui regressões; a API 334 já existente `native_inventory_move_item(source, destination, item, cancel)` permite cancelar a transferência por item no funil (e 352 observa o começo da tentativa). É um recurso oficial útil para a automação futura, porém seu caminho curto exige trait mods registrados (`mods().is_empty()` pula a chamada em um bundle só de mixins). O funil em `game/machines.rs::can_take` continua baseado em outputs para contêiner de mod; ainda não há política única por slot/item/dados que cubra a previsão de menu, servidor e funil.
- Fechar menus de BE: `native_close_menu` ainda recusa menus que não sejam `Slots::Native`; hooks `player_container_close` observam o fechamento. Não há helper oficial equivalente para fechar arbitrariamente o menu de BE do NTM por identidade.
- Save atômico: autosave de arquivos dos jogadores sai da thread, falhas de save são reportadas/serializadas e world_saved(wait) permanece. Nenhuma transação entre regiões/metadata e os arquivos do mod; retém-se o protocolo NTM de efeitos/recibos e suas limitações de queda abrupta.
- Entidades de mod: `EntityRef` agora fornece velocidade/on_ground/bounds/olho/distância, NBT, tags e nome, com handles por epoch; somadas a summon/displays/impactos existentes, ajudam a usar entidades vanilla. O conteúdo já permite entity_type derivado de snowball (item arremessado, dano fixo, rede e save por chunk); displays/falling blocks também já existem. São opções oficiais para projéteis simples, mas não equivalem à física, dano/térmica, recuperação e desenho dos detritos ZIRNOX. O intervalo não ampliou ENTITY_TEMPLATES além de snowball; não migrar os detritos cosméticos sem preservar esses contratos.

Compatibilidade adicional: o snapshot introduz recursion_limit no crate root, rejeitado pelo mixins ddd9700 fixado (E_UNSUPPORTED_MACRO). Removido localmente conforme o mesmo mecanismo da PR oficial ainda aberta #8193, dividindo também os JSONs aninhados do controle de mobs para compilar no limite padrão; não se avançou o snapshot. O novo gerador exige mod_api para cada hook: os três hooks locais passam por contratos/despachos oficiais, sem alterar os pontos de injeção do NTM.

| Verificação | Resultado |
| --- | --- |
| NTM | 369 unitários + 26 de integração passaram; 2 doctests continuam ignorados. |
| Conteúdo Python | 218 passaram, recursos 26.3 explícitos. |
| Motor, fonte original | 139 nativos; 36 menus; 34 render/mods; 66 conteúdo (incluindo os ignorados); 4 multiblocos; 2 i64; 3 partículas; 4 mod_api; 3 hooks tipados; 15 gerador. Todos passaram. Há sobreposição entre filtros; não somar como testes distintos. |
| Clippy e formatação | Fonte original, NTM (all-targets) e cliente release transformado passaram com -D warnings e sem isenções. rustfmt passou. |
| Updates flag 3 | Teste na cópia transformada passou: chest/redstone_block/stone comparados ao setBlock normal, conteúdo retido, luz/heightmaps, observer e lâmpada. |
| Build | Release, 31 injeções e 6 módulos, incluindo Recipes. Conteúdo reempacotado com a ferramenta desse release. |
| Mineração | 18 verificações, incluindo escavadeira, poço e torre de fracking. |
| Bombas | 49 verificações, incluindo Little Boy/redstone, detonador e pavio. Nenhum erro de decode do flash. |
| ZIRNOX | 110 verificações: montagem, inatividade, conversão, reabertura, falha e solos. Traço físico: uma explosão aplicada, centro [115,104,6], potência 12 e 72 detritos criados. |
| Pile | 83 verificações, incluindo reabertura/destruição e precondição de ar positivo pela UI normal. |
| Dimensões | 10 + 2 idle + 1 idle-clean; três mundos novos separados. |
| Catálogo K3 | 340 verificações e 94 itens. |
| Solar/Stirling | 15 verificações e 21 estados idênticos após encerrar/reabrir. |
| Complementares em jogo | Prensa elétrica 7; eletrolisador 4; efeitos/dano de radiação 2. |
| Recibos | Em cada um dos sete mundos dos roteiros nativos, machine_effects, applied_machine_effects, removals e prepared_retentions vazios após o encerramento normal. |


Cliente: `.agent-tmp/zirnox/native-target/release/horizon-client`, SHA-256
`912c112ffbf1d6743d878b0f66e7dcf1ca3e76ad572c764f7328f4a9abc940ae`. Conteúdo: `dist/ntm.hmod`, SHA-256 `eb96b19246c029df3be847d25c1ae068f88b177940a9b48cc9b00635dbcab631`,
9.103.885 bytes (igual ao pacote da referência ZIRNOX atual).

Evidências relativas à raiz do projeto: `.agent-tmp/upstream-fc021/`, especialmente
`snapshot.json`, `validated-build.json`, `rust-results.json`, `game-summary.json`
e logs. O agregado confere as mesmas fases, identidades e quantidades de checks de
`.agent-tmp/zirnox/validation-generation.json`; os horários/ticks não são fixados. Quatro famílias de checks de UI incluem o número dinâmico de passos/polls no nome: o comparador normaliza somente esse sufixo e preserva a multiplicidade de cada asserção.
A revisão estática pelo próprio executor está no artifact `revisao-upstream-fc021`.

Os dez arquivos locais do usuário foram preservados byte a byte e ficaram fora do
commit. Foram usados somente mundos descartáveis e Xvfb isolado. Os caches de
jogo desta rodada foram removidos após extrair as provas; reutilizaram-se os
caches Cargo existentes, sem criar targets adicionais. Os avisos de TAA no
OpenGL/Xvfb continuam esperados; essa aceitação funcional/UI não valida TAA nem
substitui a suíte completa de paridade vanilla.

Pendências: incorporar futuros snapshots em outra rodada; enviar separadamente a
correção genérica do flash ao upstream; acompanhar a correção oficial da
compatibilidade recursion_limit/mixins (#8193); os contratos de stillValid por
identidade, can_take unificado, fechamento de menu de BE, transação de save e
entidades NTM continuam nos limites descritos acima. Nenhuma isenção de Clippy
permaneceu nesta rodada.


## Onda 0 — política de inventário e portas (marco 1)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c`, derivado do pin
`ec479adb3` (`47173f4c4` e quatro correções da revisão); upstream congelado `fc0210381`. [Contrato e exemplo da
API](native-slot-policy.md).

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| `ModSlotFilter::policy`, `SlotAction`, `SlotOrigin` | O hook `slot_filters` existente recebe um callback puro com inventário atual, índice local, stack completo, ação e origem Manual/Automation. Previsão, menus autoritativos, shift-click, bundle, drag, troca, descarte e funil consultam a mesma regra. `Rules::may_pickup`, `inventory_move_item`, filtros estáticos, limites e faces continuam ativos. |
| `Multiblock::inventory_ports` e `inventory_core` | Resolve apenas dummies declarados como portas até o contêiner do núcleo vivo. `Containers::view` compartilha o comportamento com funis, carrinhos e `native_insert_item`. O núcleo conserva o inventário próprio; não-portas e núcleos sem BE não expõem contêiner. |

O ZIRNOX habilita saída automática nos slots 0..23 somente quando
`zirnox::can_extract` permite; a exclusão provisória da extração foi removida.
As cinco portas seguem `ReactorZirnox.getAllPorts`. O funil vanilla entra pela
porta superior e extrai pelo núcleo inferior. Sob cada porta lateral há
invólucro, que impede a instalação física de um funil extrator também no Java.
A trilha RTG/radgen ligará suas próprias funções depois da integração deste
marco, sem cherry-picks entre branches.

### Correções da revisão independente (P3)

| # | Correção | Commit do motor |
| --- | --- | --- |
| 1 | `bundle_click` na LAN exige `may_place` na retirada parcial para a trouxa, como `menu/bundle.rs` e `try_remove`. | `3e479f8b0` |
| 2 | `swap` (teclas 1–9 e `F`) na LAN divide a pilha até `slot_max`, como `Menu::swap`: o resto fica na mão/hotbar e a pilha antiga vai ao inventário ou ao chão. | `88327ab6c` |
| 3 | Troca direta (cursor ocupado, teclas 1–9, `F`) avalia `may_place` com o slot já vazio depois de `may_pickup`, no cliente e na LAN. Mesclar o mesmo item continua vendo o slot ocupado. | `f27edd9de` |
| 5 | Os slots 0..23 do ZIRNOX são `Slot` vanilla no Java: a inserção manual é irrestrita (uma unidade). Barras ficam exigidas na automação (`slot_allows`) e no shift-click, por `ModSlotFilter::quick_move`. | `7607685b8` (motor) e `1e62763f` (NTM) |

O item 5 exigiu um campo novo no motor: sem ele, remover o `accept` estático
deixaria o shift-click mover qualquer item para as barras, o que o
`transferStackInSlot` Java não faz. Os slots 24 e 25 (recipientes de CO₂ e de
água) são `Slot` vanilla no Java e receberam o mesmo tratamento depois: mão
livre, só recipientes do fluido certo pelo shift-click e nada pela automação
(`isItemValidForSlot` é falso para `i >= 24`). Os slots 26 e 27 seguem
`SlotTakeOnly`.

Validação das correções (após as quatro, mesmo binário em todos os roteiros):
release `horizon-client` `539b1e144b59c4ffb173e76e946e32f800bcfeda0c903444933704b2cf11f87e`,
pacote `dist/ntm.hmod` `c2eda3dc2bc98b7f9d710b3bd3c3e3db39a84509515e7fae23c439b9a6894321`
(9.103.922 bytes, idêntico ao anterior: nada mudou no conteúdo), pin `7607685b8`.

- **Motor:** 139 `native_`, 43 `menu::tests::` (inclui 21 de `lan::menu`), 5 `mod_multiblocks`, 35 `mod_` (render), 2+4 `conditional_policy`; Clippy `-D warnings` sem isenções nas bibliotecas, `cargo fmt --check` e Clippy da cópia transformada (stage).
  Testes novos: `guest_bundle_partial_take_needs_the_slot_to_accept_placement`, `limited_slot_splits_oversized_offhand_and_hotbar_swaps`, `conditional_policy_sees_the_slot_empty_when_a_stack_replaces_it` e a versão `guest_`, `quick_move_filter_narrows_shift_click_but_not_hand_placement` e a versão `guest_`.
- **NTM:** 372 unitários + 26 de integração (2 ignorados), Clippy `-D warnings` do workspace sem isenções. Teste novo: `zirnox_manual_insertion_is_unrestricted_but_automation_and_shift_click_take_rods`.
- **Em jogo (`--scratch-root proofs`):** `zirnox` (`zirnox-pvmk0r95`, 127 checks, 0 falhas, inclui `automation` e as novas `manual_non_fuel_*`: terra substitui uma barra viva no slot 300, a barra volta inteira, a terra volta ao inventário e a vida da barra não muda), `inventory-proxy` (`inventory-proxy-29egtrei`, 12), `bombs` (`bombs-43v377jt`, 49), `pile` (`pile-p90_s6q8`, 83), `dims` (`dims-s28w2hfs`, 10), `dims-idle` (`dims-idle-jsq4d1ed`, 2), `dims-idle-clean` (`dims-idle-clean-1fsb1cwp`, 1), `mining` (`mining-yzmarqd8`, 18) e `general.py` (`general-x18sezpx`, catálogo K3 340 checks/94 itens + 49 de prensa, eletrolisador, radiação, solar/Stirling e reabertura).
  Uma execução anterior do `zirnox` (`zirnox-2muh4e3g`) falhou no roteiro novo, que procurava um slot de barra vazio quando os 24 estão cheios; o roteiro foi corrigido para substituir a barra do slot 300 e a execução acima é a do roteiro final.
- Limite: não há roteiro de dois clientes, então a LAN foi coberta só pelos testes de `lan::menu`; nenhum roteiro em jogo exerce o hóspede LAN.

Registro anterior ao pacote de correções (binário `515b953d702e…`, pin `47173f4c4`):
`inventory-proxy-iz5l5y6f` (12 checks), `zirnox-2xubdn0h` (123 checks, com a primeira
prova de `automation`: 11 `zfb_mox_depleted` + 1 trítio extraídos, 0 no binário
anterior `912c112ffbf1…`), `mining-50ih28gu`, `bombs-jjh4t8wg`, `pile-9ibhz_3e`,
`dims-34f1wlwd`, `dims-idle-k1pkjqvu`, `dims-idle-clean-dnrbdizz` e `general-dnj_x0kf`.


## Onda 0 — itens 2 a 4 (menu por identidade, itens estacionados, formação da pilha)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c` (sobre `7607685b8`); NTM rebaseado em `fb93c7cf` (RTG/radgen).

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| `BlockEntities::identity` (`horizon-client-render`) | Número dado a cada objeto de block entity: mantido na reescrita no lugar e no `take`/`put`, perdido na remoção, na troca de tipo e no descarregamento; nunca reutilizado. É o `level.getBlockEntity(pos) == this` do `stillValid`. `BlockEntity::fits` (#7973/#7880) continua responsável por trocar BEs incompatíveis com o bloco; a identidade cobre a troca por outro objeto do mesmo tipo. |
| `lan::menu::Menu::bound`, `Hub::bind_menu`, `Menu::valid` e `Menu::still_valid` | O menu aberto guarda as identidades dos BEs da abertura (`lan_actions.rs`, depois de `open_container`). Cliques, botões (`mod_menu_button`), shift-click nativo e `sync_containers` exigem a mesma identidade; um menu cujo BE foi removido e recolocado fecha e seus pacotes em trânsito são ignorados. Veículos e menus avulsos não são fixados. |

Prova no motor: `identity_follows_the_object_not_the_position` (unidade) e `a_window_pinned_to_a_replaced_block_entity_takes_no_clicks`
(`tests/lan_bot.rs`, hub real: a janela antiga recebe um clique depois de quebrar e recolocar o mesmo baú e não muda nada; sem o
`bind_menu` o teste falha em `the old window is stale`; uma janela aberta depois funciona). Em jogo (`native_dims.py`, `identity_close`):
com `tick freeze`, o menu de um forno elétrico fecha quando ele é quebrado e outro do mesmo id é colocado, e o menu da máquina nova abre.

Item 3 (itens em nível estacionado): ver `host-dimensoes.md`. Item 4 (formação da pilha): `Server::pile_form` monta as escritas
(tijolo → bloco, primeiro JSON e dados do BE novo em `CellWrite::initial`, desgaste da broca em `wear_held`) e `Host::commit_formation`
as publica em `publish_atomic`, sob o acesso exclusivo ao `Game`. Preflight: células ainda tijolos simples, broca ainda na mão.
Recusa deixa tijolos, modelo e ferramenta como estavam (`formation_publishes_blocks_first_state_and_wear_together_or_not_at_all`,
`formation_with_an_unloaded_cell_changes_nothing`). Em jogo (`native_pile.py`, `formation_wear`): uma broca comum forma uma pilha em
sobrevivência e perde exatamente 1 de durabilidade.

Validação (mesmo binário `a79e93f2f051b30ad48a998b922d3a62f219718f7087f6382e2403b6895a9287`, pacote `b18682da6df57763ecf5955ca9bdc276c4cb314661aebb3ee7af0f3ee9d8f06e`):

- Motor: 139 `native_`, 43 `menu::tests::`, 18 `lan_bot`, 5 `mod_multiblocks`, 35 `mod_`, 7 `block_entities`, 2+4 `conditional_policy`; Clippy `-D warnings` e `fmt` limpos.
- NTM: 389 unitários + 26 de integração (2 ignorados); Clippy do workspace e do stage transformado sem isenções.
- Em jogo (`proofs/`): `dims-g3eloyq0` 14, `zirnox-cg0pza3j` 133, `inventory-proxy-vfu15rte` 12, `radioisotopes-sr5ezwd4` 67 (setup, decay, reopen e automation), `bombs-t6g29o6c` 49, `pile-h_lkne1n` 85, `dims-idle-oidsl1u7` 2, `dims-idle-clean-1dhjpnjg` 1, `mining-haaayl08` 18, `solar-cy9s9yup` 36, `combustion-ywdes7bf` 15, `catalog-4dth6s9s` 251. Nenhuma falha.
- Limites: o menu fixado só é exercitado por pacotes reais no `lan_bot`; em jogo o teste fecha o menu do jogador local. O item 3 não tem leitura de comparador (ver `host-dimensoes.md`).


### Correções da revisão do marco 2 (P3)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c`; NTM `wave0-integrity` com `662c5bf9` e `0364228a`.

- **Comparadores (P3 #1).** A bateria, o tanque de fluido e o barril de aço (os portados cujo Java tem `hasComparatorInputOverride`)
  respondem à API 64 (`has_analog_output`, `analog_output`) com as fórmulas do Java, e o glue chama `native_update_output_neighbors`
  quando o sinal muda num commit de máquina, também em nível estacionado (`visit_level`). Ver `host-dimensoes.md`. As demais máquinas
  com a propriedade no Java (BAT9000, tanque grande, soquete, caixas, Geiger, controle MK1) ainda não foram portadas.
- **Shift-click do ZIRNOX (P3 #2).** `ModSlotFilter::quick_inserts` deixa um slot `output` que nomeia a lista receber o item pelo
  shift-click; o CO₂ vai ao slot 24 e transborda para o 26, a água ao 25 e depois ao 27, como o `mergeItemStack` do 1.7.10.
  Testes: `quick_move_reaches_an_output_slot_that_names_the_item` (e a versão `guest_`), `zirnox_manual_insertion_is_unrestricted_but_automation_and_shift_click_take_rods`;
  em jogo, `shift_click_overflows_into_the_take_only_slot`.
  O Java também enche as barras de trás para a frente (`mergeItemStack(0, 24, true)`); o motor enche da frente. Não alterado (sem efeito
  de jogabilidade: todas as células aceitam qualquer barra).

Validação (binário `dc0eea70c23dd42d85f4497a865867d20114bd90089c1ce91dfbc16c67d2f172`, pacote `b18682da6df57763ecf5955ca9bdc276c4cb314661aebb3ee7af0f3ee9d8f06e`):

- Motor: 139 `native_`, 45 `menu::tests::`, 18 `lan_bot`, 7 `block_entities`, 35 `mod_`, 5 multiblocos, política condicional; Clippy `-D warnings` e `fmt` limpos.
- NTM: 416 testes (389 unitários + 27 de integração), 2 ignorados; Clippy do workspace e do stage transformado sem isenções.
- Em jogo, sem falhas: dims 14, zirnox 134, inventory-proxy 12, radioisotopes 67, bombs 49, pile 85, dims-idle 2, dims-idle-clean 1,
  mining 18, solar 36, combustion 15, catalog 251, comparator 7.


### Fechamentos adicionais da revisão (ordem das barras e rede de baterias)

Motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c`.

- **Ordem do shift-click.** `ModSlotFilter::quick_reverse` marca slots que um shift-click visita de trás para a frente dentro de uma
  corrida de slots consecutivos (`quick_visit_order`), como `mergeItemStack(0, 24, true)`; os slots fora da corrida seguem a ordem
  normal. O ZIRNOX marca os 24 slots de barras. Previsão e host LAN usam a mesma ordem (testes
  `quick_move_fills_a_reverse_run_back_to_front` e a versão `guest_`); em jogo, `shift_click_fills_the_rod_slots_back_to_front`.
- **Baterias não trocavam energia pelo cabo (causa e correção).** A rede do port (`energy::distribute`) fazia três passos:
  geradores→consumidores, geradores→armazenamento, armazenamento→consumidores. Nunca armazenamento→armazenamento, então duas
  baterias no modo Buffer não trocavam nada. **Isso não é fiel ao Java.** `PowerNetMK2.update` junta todos os provedores
  (`min(power, getProviderSpeed())`) e todos os receptores (`min(max-power, getReceiverSpeed())`) num só conjunto, e
  `TileEntityBatteryBase.updateEntity` põe uma bateria em Buffer nos dois conjuntos (`addProvider` e `addReceiver`);
  `TileEntityMachineBattery.getProviderSpeed` é `max/600` e `getReceiverSpeed` é `max/200`. Uma bateria cheia alimenta uma vazia.
  Acrescentei o quarto passo (armazenamento→armazenamento, só com duas ou mais, para não girar sobre si mesma), com as mesmas
  taxas. Como no Java, duas baterias com a mesma oferta e o mesmo espaço pesam igual e param de trocar. Teste:
  `batteries_exchange_energy_with_each_other`; em jogo, a segunda bateria do `native_comparator.py` sobe a partir de 0 enquanto a primeira desce.
- **Observação, não alterada:** no Java a bateria nova começa com `redLow = 0` (entrada apenas) e `redHigh = 2` (saída com redstone);
  o port começa em Buffer/Saída. Mudar isso quebraria os fluxos existentes (a bateria criativa alimentando máquinas sem redstone);
  fica registrado para uma decisão de produto.

Validação desta rodada (binário `ec4feef1ac29177e5dd6f126cdf1229e2d77fa857b737e41b2478bac6f25966c`, pacote `b18682da…f06e`):

- Motor: 139 `native_`, 47 `menu::tests::`, 23 de `lan::menu`, 18 `lan_bot`, 7 `block_entities`, 35 `mod_`; Clippy `-D warnings` e `fmt` limpos.
- NTM: 417 testes (2 ignorados), Clippy do workspace e do stage sem isenções.
- Em jogo, sem falhas: dims 14, zirnox 136, inventory-proxy 12, radioisotopes 67, bombs 49, pile 85, dims-idle 2, dims-idle-clean 1,
  mining 18, solar 36, combustion 15, catalog 251, comparator 9 (inclui a transferência bateria→bateria).
- O `pile` falhou uma vez em `meltdown_dismantles_pile`; a causa está abaixo (era o roteiro) e não é mais intermitente.

### Fechamentos finais: bateria como no Java e o meltdown do `pile`

NTM `wave0-integrity` sobre o motor `d5eb69a33a5c2d137d620865dd1e39e1e31aa38c` (sem mudança no motor nesta rodada).

- **Modo padrão da bateria.** `TileEntityMachineBattery` nasce com `redLow = 0` (só entrada) e `redHigh = 2` (saída com redstone); `Battery::new`
  agora também. O `native.rs` teste `the_battery_fills_from_its_slot_and_turns_its_modes` confere o estado inicial `[.., 0, 2, ..]` e os cliques
  da tela (`redLow`: entrada → buffer → saída → nada). A bateria criativa é um item (`IBatteryItem`), sem modo próprio: não mudou.
  Os roteiros que usam uma bateria como fonte agora fazem o que um jogador faz: um bloco de redstone sob a bateria
  (`Game.battery_output`) em `native_dims`, `native_comparator`, `native_stations`, `native_redes` e `native_recipes`.
  As que só armazenam (combustion, solar/Stirling, RTG/radgen) seguem como estavam.
- **`pile` / `meltdown_dismantles_pile` — causa.** Não é defeito da varredura de órfãos. Evidências:
  1. O teste de unidade `native_meltdown_uses_the_engine_blast_and_never_drops_rods_twice` agora exige que **todo** `pile_block` reverta no tick
     seguinte ao do núcleo (antes conferia uma célula).
  2. A sonda `pile-meltdown` (novo cenário; servidor congelado, `tick step` de 5 em 5 até o núcleo reverter, depois de 1 em 1): em 5 rodadas o
     varrimento reverteu as 101 células de pé exatamente **um tick** depois do núcleo, e as demais vezes já estava feito dentro do mesmo passo de 5.
  3. O tick do estouro (explosão de potência 15 mais as atualizações de bloco) prende o servidor integrado por vários segundos: leituras de
     células a cada 40 ms viram 42 a 75 `pile_block` de pé durante os primeiros 5 s e zero depois, e `time query` chegou a passar de 10 s sem resposta.
     O roteiro lia o mundo 3 s (relógio de parede) depois de ver o núcleo reverter, antes de o tick do varrimento rodar.
  Correção do roteiro: `destroy` espera o varrimento (até 90 s) em vez de dormir 3 s; `Game.ctl` repete um comando que o controlador deixou
  sem resposta por causa de um tick pesado; a sonda `pile-meltdown` fica como regressão por ticks.
  Observação de desempenho, fora do escopo: o estouro do meltdown prende o servidor por mais de 10 s neste host (render por software).
- **Execuções do `pile`.** Cinco seguidas no binário final, `pile-b1` a `pile-b5`, todas com 85 checks e sem falha, mais cinco antes do ajuste
  do `ctl` (quatro passaram; a quinta caiu num `execute` que o servidor ocupado não respondeu em 10 s, o defeito do `ctl` corrigido acima).
- `native_recipes.py` e `native_stations.py` não fazem parte da bateria padrão; `stations` roda (5 checks); `recipes` (montadora) falha
  na abertura do menu com ou sem esta mudança (verificado por A/B), por posicionamento do próprio roteiro antigo.

Validação (binário `e57ab62508c8c7c9bbdf5283fc3794882b462b0d9654c23e8aefc9a8fdf5f3d4`, pacote `b18682da…f06e`):

- NTM: 417 testes (2 ignorados), Clippy do workspace e do stage sem isenções. Motor sem mudança desde `d5eb69a33` (testes da rodada anterior).
- Em jogo, sem falhas: dims 14, zirnox 136, inventory-proxy 12, radioisotopes 67, bombs 49, pile 85 (×5), pile-meltdown 32, dims-idle 2,
  dims-idle-clean 1, mining 18, solar 36, combustion 15, catalog 251, comparator 9, stations 5.

## Onda 0 — item 5 (save atômico)

Branch NTM `wave0-vanilla` sobre `main` `85d06d9c`; motor `wave0-vanilla` `a31497f38` sobre `ntm-current` `f22df7b81`:
instrumentação de teste (os failpoints do save, o alvo `live_chunk`, `HORIZON_WRITER_HOLD`) e duas extensões locais
(`world_saving` e a descarga com o chunk no lugar; seção seguinte). **Estado: entregue para revisão independente, sem
merge.** A sétima revisão e a segunda rodada da revisão fresca reprovaram o R6 (R7-1 a R7-4, N1, N2); todos foram fechados
na raiz. Backups: NTM e motor `wave0-save-r4c` (`891d02f0`, `1d48f96aa`).

**Critério.** Cada execução é comparada com o vanilla no mesmo roteiro e no mesmo corte, **item por item**: (L) nada some
que o vanilla guarde; (D) nenhum item fica com mais cópias que no vanilla. Nenhuma violação é dispensada por categoria.

**Princípio: toda obrigação mora no registro do chunk que a originou, nunca no KV** (a semântica de um BE vanilla). E nada
é dado a um registro depois que um save o tomou.

| Parte | Como fica |
| --- | --- |
| Armazenamento (KV, `data/ntm/state`) | Em memória; gravado inteiro no fim de cada save (`.tmp` e `rename`; `fsync` com espera), depois de `native_world_saved(wait)` com o `wait` real do save (R7-1). Guarda só o que é do mundo e não decide o conteúdo de nenhum chunk: bombas, radiação, ticks, zirnox, gerações. **Nenhuma remoção, lote de efeitos, recibo ou sobrevivente** de uma versão nova. |
| Obrigações no registro do chunk (`server::obligations`) | Uma remoção que espera seu grupo e um lote de efeitos em voo moram no anexo `ntm:ledger` da sua casa: o chunk da máquina removida, ou o do primeiro dono do lote (o núcleo numa pilha). O servidor dá a cada registro o que ele deve no mesmo tick em que muda (`store_obligations`), e só joga o que uma casa carregada deve (`home_loaded`); quando ela carrega, o registro diz o que ainda deve (`native_obligations_loaded`). |
| Mudança entre ticks (N1, R7-2) | Um bloco mudado depois do último tick (um comando, um tick congelado) é tomado como o fim de um tick o toma (`native_settle`: eventos, só os passos dos grupos cuja máquina ou núcleo saiu, efeitos, obrigações): antes de o save tomar qualquer coisa (`world_saving`) e na descarga, com o chunk ainda no lugar. O conteúdo de uma máquina removida sai na hora, como o de um baú. A regravação tardia de registros depois das regiões foi removida. |
| Quebra de um núcleo | Solta as hastes na hora, do chunk do núcleo, como o `breakBlock` do Java; membros fora de alcance voltam a tijolo quando carregam (varredura de órfãos). Nenhuma remoção espera segurando itens. |
| Nomes dos lotes (R6-1) | Derivados de um nonce sorteado por sessão e de uma contagem: um KV que volta com uma morte não repete o nome que um registro ainda guarda como pago. |
| Emissão | Chunk do drop carregado: drop vanilla no mesmo passo. Não carregado: devido no registro do dono (o slot de saída), pago quando o chunk entra. |
| Multibloco rasgado | O núcleo com uma célula estranha se desmonta; membros sem núcleo voltam a tijolo (`a_pile_saved_formed_on_one_side_of_a_border_reopens_whole_or_as_bricks`). |
| Saves antigos: o ledger | Migração única e idempotente do ledger rastreado (R5-1); um registro ainda não resolvido atravessa podas e saves inteiro (F2) e sua contagem é ajustada pelo parser SNBT (F3). |
| Saves antigos: remoções e lotes do KV (R7-1, N2) | Ficam no KV **como foram lidos** (`WorldState::legacy`), não como a fila está, e nenhum é jogado antes de o registro da casa ser lido (`ServerHost::home_migrated`). Um registro que já o tomou (`migrated: X`, guardado com o registro) diz o que resta dele, e a cópia do KV não acrescenta nada. Senão o servidor o assume e o registro passa a dizer `migrated: X`. O KV o descarta só quando um save **com espera** gravou o registro com o marcador antes dele (capturado em `world_saving`), ou quando o registro foi lido do disco com o marcador. Um save sem espera não aposenta nada. O marcador sai quando o KV aberto com o mundo já não o nomeia. Um lote antigo sem nome recebe um derivado do seu conteúdo, igual em toda leitura. |
| Recibos e sobreviventes da sessão | O que um save levou com os chunks é confirmado quando os registros estão no disco: na hora, num save com espera; num sem espera, no primeiro tick em que o escritor do motor diz que tudo o que recebeu está no disco (`writes_finished`). Achado do meltdown no R7b: confirmados só por save com espera, cresciam sem fim numa sessão de autosaves, e o passo de cada grupo os copiava (pior tick de 61 ms na sexta rodada). |
| Bombas (F4, R7-3) | Fiel ao HBM 1.7.10, quirks incluídos: a cratera de uma nuke (`EntityNukeExplosionMK5`) para com o mundo, e o balefire (`EntityBalefire`) continua. O dano de cada uma dura exatamente a sua cratera (o pulso guarda o id da explosão), curta ou longa, sem prazo fixo; ver `docs/bombas.md`. |

**Verificador** (`crates/ntm-core/tests/save_model.rs`). Chunk A da máquina (BE, devidos, nomes pagos, marcador), funil,
um terceiro chunk, os arquivos de entidades, o jogador, o KV gravado no instante do save com as regiões ainda na fila, e o
escritor do motor. A máquina atira duas vezes (em sessões diferentes) e pode ser quebrada: por um jogador (espalha na hora,
como o dropper vanilla) ou por um comando entre dois ticks (`Command`), que o mod só toma no fim do próximo tick, antes de
um save tomar os chunks, ou numa descarga com o chunk no lugar. Um tiro devido pago deixa seu nome no registro de A. O
mundo também abre de um KV antigo com uma remoção ou um lote. O análogo vanilla da remoção é um baú inteiro no registro de
A, quebrado no instante em que o mundo lê esse registro; o do lote é a saída de um dropper. Cada sequência roda no mod e no
vanilla, passo a passo; (L) e (D) por átomo a cada estado, sem exceções.

| Execução | Resultado |
| --- | --- |
| N=9 | 188.661.834 estados visitados, 12.399.394 distintos, 8.361.560.872 sequências; zero violações. |
| **N=10** | **1.437.623.114 visitados, 92.279.811 distintos, 116.107.959.595 sequências; zero.** 10.783 s. (O arquivo do log foi sobrescrito por uma segunda execução, interrompida; o resultado foi restaurado do log da shell monitorada, com nota.) |
| KV antigo (remoção e lote) | Profundidade 9: remoção 137.589.262 visitados, 8.415.470 distintos, 9.804.079.312 sequências; lote 279.953.704 visitados, 17.630.320 distintos, 10.284.021.767 sequências; zero. |
| Passeios aleatórios | 6 milhões de até 40 operações (240 M operações) e, com outra semente, 6 milhões de até 60 (360 M); zero. Os passeios acharam uma divergência do próprio modelo (o nome do derrame de um comando), minimizada por delta-debugging a 10 operações e corrigida no modelo. |
| Morte fora de um save | Os dois mundos voltam ao último save, átomo por átomo. |
| Mutante `InStorage` (devido no KV) | Pego em 129.834 estados. |
| Mutante `RemovalInStorage` (R6-2, F1) | Pego em 310.728 estados. |
| Mutante `CounterIds` (R6-1) | Pego em 43.155.679 estados. |
| Mutante `PublishAfterCapture` (N1: publica depois da captura) | Pego em 357 estados: `[Fire(A), Command, Save(true), Kill]`. |
| Mutante `UnloadWithoutHome` (R7-2: unload sem casa) | Pego em 33.775 estados: `[Fire(A), Command, Unload(A), Unload(funil), Save(true), Kill]`. |
| Mutante `RetireEarly` (R7-1: migração aposenta antes) | Pego em 137 (remoção) e 138 (lote) estados: `[…, Save(false), Kill]`. |
| Mutante `NoMarker` (N2: sem o marcador) | Pego em 6.606 estados: `[…, Pickup, Save(false), Write, Kill]`: o estoque sai duas vezes. |

O verificador achou duas divergências reais no caminho. A primeira (R6): com a remoção que esperava o grupo, o mod
guardava as hastes no registro de A, enquanto o vanilla as espalhava nas entidades. Daí a quebra imediata. A segunda (R7),
no próprio desenho da migração: a primeira versão comparava a remoção antiga com hastes já espalhadas nas entidades de A, e
a descarga (entidades, depois registro) dava duas cópias. O análogo certo é o baú inteiro quebrado na leitura, que tem o
mesmo resíduo de ordem da descarga vanilla. Os sete mutantes ficam como testes permanentes. Os testes de unidade
(`pile_server`: `a_legacy_*`, `a_marker_read_from_the_disk_*`, `a_break_not_yet_taken_when_its_chunk_unloads_*`,
`a_break_with_no_tick_before_the_save_*`) cobrem cada corte com o código real, e cada um falha com a mutação
correspondente aplicada ao código (aposentar sem espera, ignorar o marcador, não acomodar antes do save).

**Provas em jogo.** **129/129** pontos em 24 cenários no binário final, duas reaberturas, cada uma comparada **por
item** (também a segunda, R7-4), cada cenário jogado também no controle vanilla. Todo cenário tem `kill` (morte fora de
um save) e **`kv_committed`**: um save sem espera (a pausa) com o escritor de regiões parado (`HORIZON_WRITER_HOLD`) e o
processo morto logo depois do KV (`NTM_FAILPOINT=kv_committed`), o corte "KV novo, regiões antigas". O oráculo compara o
**multiconjunto** de itens por id (hastes por tipo, devidos e remoções dos anexos incluídos) no disco do corte, depois de
cada reabertura e contra o controle, e exige `kv_items = 0` no disco dos dois mundos. Um corte por índice de registro é
comparado com o controle que gravou os mesmos chunks, em todas as condições (F5).

| Grupo | Pontos | NTM = controle |
| --- | --- | --- |
| `original` (todos os cortes, `kv_committed`, `live_chunk:2,0`/`3,0`, varredura) | 18 | 12, ou 2 quando o núcleo chegou sem as entidades (os mesmos chunks gravados no controle) |
| `mining` (todos os cortes, `kv_committed`, `live_chunk:1,0`) | 16 | 7, ou 0 em `chunks_written`/`live_chunk:1,0`, como o baú |
| **`frozen-break`** (N1: núcleo quebrado por comando com ticks congelados, save sem tick no meio) | 7 | 12 = baú em todos os cortes; 2 = 2 em `live_chunk:2,0`/`chunks_written` (as hastes nas entidades ainda não gravadas, como os itens do baú) |
| **`frozen-unload`** (R7-2: a mesma quebra, e o chunk do núcleo descarrega antes de um tick) | 3 | 12 = baú |
| **`legacy`** (R7-1 e N2: KV no formato R5 com um lote devido ou já entregue, cortado por `kv_committed`, por `players_saved`, regiões novas com KV antigo, ou inteiro) | 6 | 1 niter em cada uma das duas aberturas, por item, e depois de pago; o KV do corte o guarda (só um save com espera o aposenta) |
| `coupled-a/b/ea/eb` | 24 | iguais ao funil em cada corte |
| `coupled-break` (R6-2) | 6 | 6; em `chunks_written`/`entities_written` o mod guarda as hastes devidas no registro que o vanilla perde nas entidades não gravadas (6 contra 1), sem cópia por item |
| `break`/`replace`/`break64` | 21 | iguais ao baú (16 ou 64; 0 nos cortes que gravaram só o chunk do baú) |
| `save-quit`, `crossing`, `crossing-kill` | 8 | 12; 24 e 20 = controle (entidade que atravessou para um chunk descarregado) |
| `origin-*`, `merge-stale`, `pickup-merge`, `origin-away` | 20 | iguais ao controle; em `kv_committed`, 32 e 176: o arquivo do jogador gravado pela pausa à frente das regiões (resíduo vanilla) |

Rodadas em paralelo (até 10 jogos), cada falha reexecutada sozinha antes de contar: 2 provas (`origin-pickup-away`,
`origin-away`) falharam por tempo no paralelo e passaram sozinhas.

**Resíduos, todos iguais ao vanilla na mesma execução:** o save rasgado entre chunks; o arquivo do jogador à frente das
regiões; a transferência rasgada na borda; o devido que volta com o estoque depois de uma morte; a entidade que
atravessou para um chunk descarregado; a descarga rasgada entre o arquivo de entidades e o registro (a do baú quebrado,
também para a remoção de um KV antigo assumida na leitura). Obrigações de uma dimensão estacionada chegam quando ela volta
a ser a ativa e o chunk recarrega.

**Desempenho do meltdown** (seis rodadas, `HORIZON_SPIKES`, pior tick por rodada em ms):

| Execução | Rodadas |
| --- | --- |
| B1 (base) | até 2.273 |
| R4c | até 359 (espera do disco no KV por tick) |
| R5f | 13/48/71/15/65/68 · 14/43/64/18/46/70 · 13/38/58/13/45/111 |
| R6f | 8/19/28/7/19/33 · 5/18/24/6/14/25 · 6/14/22/6/17/26 |
| R7b | 7/17/31/43/53/61 · 7/18/25/38/48/59 · 8/18/28/42/47/61 (cresce a cada rodada: os recibos, corrigido no R7c) |
| R6b, mesma janela | 5/14/25/6/15/24 · 8/20/29/7/22/32 |
| **R7c**, intercalado com o R6b | **9/22/31/8/23/32 · 8/20/29/7/20/28** |

As medições de R6b e R7c da última linha foram intercaladas (R6b, R7c, R6b, R7c), numa janela em que a coordenação parou
as outras frentes: nenhuma compilação nem outro jogo, carga média de 1 minuto entre 1,8 e 6,5 no início de cada uma (o
próprio jogo largo, nas CPUs 0–4 e 6–10). Os números absolutos não se comparam com os do R6f, medido noutra configuração
da máquina; o que decide é a tendência: o R7c cai a cada autosave (quarta rodada) como o R6b, e fica na faixa dele.
Medições anteriores sob a carga de outras frentes (até 13,8, com compilações) deram picos nos dois binários, como um
`keep_world` de 501 ms no R6b, e não entram na conclusão.

**Validação final.** Motor `a31497f38`; binário `horizon-client.R7c` SHA256
`98a85002ec50c70a0ac860ea3ef60618e5ed2d8c6fa924cc83c0a5d517b24ed0`, construído de `9c4061a5`. NTM: 522 testes em debug
(os do verificador só em release), verificador em release (11 testes); motor: 10 testes de `save::`/`entities::` e o da
ordem do save, 3 de `world::` (o da descarga) e 4 de ganchos; Clippy `-D warnings` do workspace NTM, do
`horizon-client-lib`, do render (lib e testes) e do cliente transformado. Regressão no mesmo binário: 28 execuções, 30
mundos, **1.943 checks, nenhuma falha** (13 execuções falharam por tempo no paralelo e passaram sozinhas). Código de
produção NTM (`crates/ntm-core/src` e `native`, com os testes de unidade): contra o R6b (`a2003c95`) 16 arquivos,
+800/−205; contra `wave0-save-r4c` 19 arquivos, +2010/−677. Motor contra `f22df7b81`: 7 arquivos, +278/−14.

Evidência local: `wave0/crash-reports/` (`R7cf-proof-summary.json`, `R7c-regression-summary.json`, `P-R7c-*`, `P-AB-*`,
`P-R6b-ab`, `P-R7b-*`), `wave0/opus/model-R7b-{9,10,walks2}.log`. Mundos aprovados são apagados após guardar logs e números.

## Onda 0 — item 5: extensões locais do motor

Motor `a31497f38` sobre `f22df7b81` (`0f21c07f0` era só instrumentação). Ambas são candidatas a PR upstream. O estudo do
upstream mostrou que `host-bridge` e `chunk-tickets` mexem perto delas: conferir esses dois na próxima atualização antes de
reaplicá-las.

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| Gancho `world_saving(wait)` (`native_world_saving`, em `native-hooks.json`), na cabeça de `save_game_with`, antes de `save_level`, dos níveis estacionados, dos metadados e dos jogadores | O upstream só tem `world_saved` (API 306), depois de tudo tomado. Todo save (autosave, pausa, `save` do controle, `/save-all`, fechamento, dedicado) passa por `save_game_with`, e nada do motor roda entre um comando do controle e esse save; um tick congelado nem chama o `server_tick` do NTM. Sem o gancho, a quebra de uma máquina com conteúdo oculto ficava fora do save que a seguia (N1). Custo nulo sem mod: um despacho vazio por save. Testes: `a_save_is_heard_before_it_takes_anything` (ordem em `save_game_with`) e `world_saving_reaches_the_mods_with_its_wait` (despacho). O efeito ponta a ponta é a prova `frozen-break`. |
| Descarga com o chunk no lugar: `native_chunks_changed` devolve a `world.chunks` as colunas que o `streaming.update` já tirou (`World::put_back`) durante o despacho de `chunk_unload` e as tira de novo, com o que os ganchos escreveram, para o `unloaded` que é gravado (`World::take_back`) | O upstream retira as colunas antes de avisar os mods, e o que um mod gravasse no chunk que sai não chegava ao registro gravado (R7-2). O `ChunkEvent.Unload` do NeoForge vem com o chunk ainda acessível, antes do save. Custo: duas operações de mapa por coluna descarregada. Teste `a_column_put_back_for_its_unload_leaves_with_what_was_written_into_it` (o anexo e o bloco escritos durante o gancho estão no chunk gravado). Prova em jogo `frozen-unload`. |

## Hooks de item — extensões locais

| Extensão local | Equivalente no upstream congelado `fc0210381` | Decisão |
| --- | --- | --- |
| `save_live_later` e `HORIZON_FAILPOINT=live_chunk:x,z` | O writer já distingue chunks vivos de protos; os ordinais antigos contam ambos. | Diagnóstico aprovado pelo coordenador. O alvo vai primeiro em sua região e o processo cai após seu header, somente no lote vivo de `save_level`. Desligado por padrão; ordem normal e gameplay preservados. O harness exige no `.mca` exatamente um dos chunks novo e o outro antigo. |
| `native_player_transactions`: snapshot por UUID/época, CAS de inventário completo, armadura, offhand e attachment com codec; fome, cura, efeitos e cooldown após validação | Hooks oficiais de uso/equipamento/dano e attachments já existem. Não oferecem commit conjunto nem recusa sem consumir parte do inventário. | Extensão local; reutiliza persistência de attachments, efeitos, cooldowns e eventos oficiais. Nenhum tipo do NTM entra no motor. |
| `remote_open_screen`: id validado do bloco de origem no BE temporário de menu customizado | O upstream deixa o id vazio e perde os filtros condicionais no caminho de previsão. | Preservar o id apenas quando o bloco clicado declara o mesmo menu; políticas compartilhadas seguem idênticas no servidor e no cliente. |
| `Hub::sync_containers` e `Menu::sent_carried`: o cursor que o mod publica depois de um clique em menu nativo | O upstream reenvia o menu e o cursor antigos quando o clique vai ao mod (`resend_menu`) e só compara slots na sincronização do tick. Um cursor alterado pelo commit do mod nunca chegava ao cliente. | Extensão local, restrita a `Slots::Native`: o hub guarda o último cursor enviado e manda `SetCursorItem` quando difere. Teste `a_native_menus_cursor_published_after_the_resend_is_synced`. |
| `Game::native_use_claimed` e `Hub::send_inventory` depois de um `UseItem` | O cliente do hóspede prevê o uso vanilla (equipar a armadura) antes de o hook nativo do servidor tomar o uso, e nada corrigia a previsão. | Extensão local: quando um mod toma o uso, o servidor reenvia o inventário completo do hóspede. |
| `NativeDropMode` e `Game::native_drop_item`: mão, lançado (`func_146097_a(.., false, false)`, com a estatística) e `EntityItem` no mundo | O upstream tem `Player.drop` (mão) e `throw_from_eyes(.., false)`, sem a estatística nem o `EntityItem` com o salto do construtor, nem escolha de modo por chamador. | Extensão local; os commits de jogador publicam uma lista de `NativeDrop`. Fórmula do salto em `world_item_velocity`, com teste; origem verificada no decompilado do 1.7.10 (`docs/native-item-hooks.md`). |
| `Menu::native_resend` e `Hub::flush_native_resends` | O hub reenviava o menu e o cursor *antes* de o mod responder ao clique (estado anterior ao clique), e o estado novo só saía no tick seguinte. | Extensão local: o clique só marca; depois de `run_native_menu_events` o hub envia o menu e o cursor já com a resposta, no mesmo tick. Teste de hub. |

Duração/animação/start/finish e tick de jogador usam exclusivamente hooks oficiais.
A aceitação desta extensão e os limites estão em `docs/native-item-hooks.md`.


## Correções da montadora e do meltdown da pile

Branch `fix-assembler-meltdown`, rebaseada sobre `main` @ `4bb53ca7`; motor `0615f5db0` (`ntm-current`, hooks de item), upstream congelado `fc0210381`.

### A montadora não abria o menu: era o roteiro

O commit `6e3f3f1e` passou a abrir o menu pela célula sul do núcleo (z=10). `Game.open` fica 3,5 blocos ao sul dela (z=13,5), mas `Game.ground` só põe o piso até z+4=12: o jogador caía de y=100 para y=76, fora de alcance, e o menu nunca abria (o roteiro leu `status.player` por 1,5 s depois do `tp` para provar isso). O item na mão e a latência foram medidos e descartados (abre em 0,14 s com e sem o item). Nada no runtime ou no menu estava errado.
`Game.ground` ganhou `depth` (padrão 4; só a fase `montadora` usa 6) e `recipes` roda `montadora` e `bigorna`, de volta à regressão.

### O estouro do meltdown prendia o servidor por mais de 10 s: diário gravado n vezes

A explosão (`native_explode`, potência 15) custa pouco. O tick ficava em 8 a 12 s de relógio com 0,4 a 1,5 s de CPU e a lista de spans vazia: a thread do servidor ficava em estado D na syscall `rename` (`folio_wait_bit`, `blk_mq_get_tag`, `ext4_wait_block_bitmap`).
`Server::flush_machine_effects` regravava o diário inteiro (`world`, 1,9 a 3,9 MB; `write` num `.tmp` e `rename`) **antes de cada efeito**. A varredura de órfãos do tick seguinte enfileira um lote com 124 efeitos `Blocks(1,1,0)`, um por membro, e gravava 125 vezes: custo quadrático (n gravações × O(n) bytes). O ambiente de teste agrava, porque `agent-run` usa `ionice -c3` em ext4/BFQ com pressão de E/S alta, mas o custo quadrático existe em qualquer disco.

**Invariante (o que permite omitir uma gravação).** Pode-se pular a gravação anterior a um efeito apenas se: (1) uma gravação da mesma chamada já guarda a fila inteira; (2) o efeito refrescado é igual ao guardado; (3) nada que uma reexecução repetiria (itens, explosão, remoção com `retained`) saiu desde essa gravação. `Blocks` simples reexecutam sem dano porque o guarda compara a célula como está. A gravação depois do último efeito continua. Quem mover o diário para outro lugar (registro do chunk, save atômico) deve manter o custo por tick independente do número de efeitos.

**Provas.**

- `the_meltdown_sweep_journals_once_and_ends_as_a_write_per_effect`: o mesmo meltdown no modo estrito (gravação por efeito, mantido só em teste por `Ready::strict_journal`) e no novo. Blocos, drops, explosão e o `world` persistido saem idênticos; a varredura vai de 125 para ≤ 3 gravações e os bytes caem mais de 20 vezes.
- `an_effect_that_must_not_repeat_is_journaled_before_the_next_one_goes_out`: depois de um efeito de itens, a gravação seguinte já tem o item como feito.

**Números** (3 meltdowns por execução do roteiro `pile-meltdown`, mesmo host, mesma carga de fundo):

| Medida | Antes (`73d7b842…`) | Depois |
| --- | --- | --- |
| Pior tick do servidor | 8,0 s; 10,5 s; 11,7 s (CPU 0,4 a 1,5 s) | 0,97 s (CPU 0,23 s); sem tick acima de 200 ms no binário final |
| "o servidor não respondeu em 10 s" | sim | nunca |
| Gravações do `world` na execução inteira | 378 (~116 por meltdown) | 75 |
| Servidor bloqueado em `rename` | 22,7 s | 1,9 s |
| Gravações no tick da varredura (teste de unidade) | 125 (134,8 MB) | ≤ 3 |

Limite: ficam ~12 gravações por meltdown vindas de outros caminhos do diário, não investigados; a CPU restante (~0,2 s no tick do estouro) não foi comparada com uma explosão vanilla de mesma potência.

### Validação

Binário `horizon-client` release `d554e57887fbd1dcf04ce9fcc460d5b362b9c87dfc0d4ab043be26a6b524e043`, pacote `dist/ntm.hmod` `3fbcad5823f75e54fd3e944780eaeec2e59f016bb7691a6e76c74919bfb3ca53` (reempacotado com o `cargo-horizon-mod` desse release). Build release em 11m39s (`SKIP_UPSTREAM_CHECK=1`).

- NTM: 423 testes passaram (2 ignorados). Clippy `-D warnings` do workspace, todos os alvos, sem isenções. Não foi executado o Clippy do stage transformado: a glue (`native/ntm/mixins`) não mudou.
- Em jogo, mesmo binário, sem falhas: items 58, recipes 5 (o JSON final é o da bigorna; a montadora também passou), pile 85, pile-meltdown 32, zirnox 136, bombs 49, dims 14, dims-idle 2, dims-idle-clean 1, mining 18, radioisotopes 67, inventory-proxy 12, solar 36, combustion 15, catalog 251, comparator 9, stations 5.
- Uma rodada anterior, sobre o binário do pin `d5eb69a33`, teve falhas por disco cheio (`/mnt/ssd120` a 100%, causado por outros diretórios de trabalho) e uma abertura de menu do `radioisotopes` sob carga extrema; repetidas com espaço livre, passaram. Esta rodada, no binário final, passou de primeira.

## Onda 1 — consumíveis, kits e medicina (K2)

Sem mudança no motor (pin `0615f5db0`). O que o NTM ganhou e onde está documentado:

| Extensão local | Razão |
|---|---|
| `ntm_core::consumables` (`consumables.json` e `kits.json`) e o glue `item_hooks.rs` | O mesmo commit atômico do jogador (`native_commit_player`) aplica a tabela de uso de cada família; ver [Hooks nativos de item](native-item-hooks.md) e [o catálogo K2](content-catalogo-k2.md). |
| `ntm_core::blast::vanilla::allocate_vnt`, `cross_hit` | `ExplosionVNT` do `chocolate_milk` pelo motor de explosões do NTM (resolução 32, força até 64, dano e empurrão do `EntityProcessorCross`); mobs e itens soltos esperam a API de entidades. |
| `Server::rad_buf`, `note_contamination` e `zone_rate` | A `radBuf` por jogador vem da mesma leitura que move a dose (sem caminho paralelo), na janela de um segundo, com os `contaminate` pontuais somados à taxa contínua: Geiger, dosímetro. |
| Mob effects de conteúdo `ntm:radx`, `radaway`, `stability`, `death`, `potion_sickness`, `taint` | Estados passivos que o Java lê com `isPotionActive`; o motor não tem tick para efeitos de conteúdo. |



## Sons próprios do HBM — 04/11/2026

Base NTM rebaseada sobre `main` `9f11740a` (inclui consumíveis K2). Motor local
`f22df7b8127ae90f2cbd66dcfe5f58be70a9032b`, derivado de `0615f5db0`;
upstream congelado `fc0210381`, sem mudança do protocolo. Pins em
`compatibility.json` e `horizon-mixin.toml` atualizados juntos.

| Extensão local | Suporte oficial reaproveitado | Razão da extensão |
| --- | --- | --- |
| `Ticking::External` no `SoundEngine` | `Sounds`, handles posicionais e atualização de volume/pitch/posição já existem | Manter instância atualizada pelo dono cliente, derivada do snapshot de BE, até stop/release explícito; não adicionar protocolo de loops. |
| `BlockProperties::door_sounds` | Loader de conteúdo e templates de `SetType` | Escolher eventos de interação open/close para uma porta declarada por um mod; validação de namespace e restrição a portas. |
| Categoria por caminho de qualquer namespace em `settings::sound_source` | Canais/categorias oficiais do áudio | O teste anterior só removia o prefixo minecraft e tratava `ntm:block.*` como master. |
| `HORIZON_TRACE_SOUNDS` opt-in, com filtro de namespace | `SoundRegistry` e decoder Ogg oficiais | Provar no log resolução para arquivo real do mod e decodificação, sem mudar o RNG normal. |
| `HORIZON_AUDIO_TEST_MIXER=1` opt-in | Mixer/decoder e control socket oficiais | Exercitar canais e PCM reais sem hardware; expor cursores, ganho/pitch e histórico de ciclo de vida só no modo diagnóstico. |

Carregamento de `assets/ntm/sounds.json`, Ogg, pesos, variantes, referências,
volume/pitch, stream, atenuação e subtitles já é oficial; não foi criado outro
registry. `native_play_sound` aceita nomes de mod por `Holder.direct`.
Os loops usam o update tag oficial dos BEs; a onda nuclear inicia por mod messages
oficiais. Detalhes, inventário Java gerado, licenças e limites em
[sons-hbm.md](sons-hbm.md). Quatro eventos excluídos por aviso Sound Dogs; CC0 de
qubodup creditado no próprio pacote. `entity.vomit` permanece silencioso como no
original, sem alias para player.vomit.

Validação após revisão: 452 unidades + 26 integrações = 478 aprovados, 2 doctests
ignorados; Clippy NTM workspace/all-targets com `-D warnings`, sem isenções.
Python: 258 contratos de conteúdo + 8 do conversor/guarda de áudio. Motor: 19
testes de backend + 8 SoundEngine; Clippy do motor/bundle lib/bins/tests passou.
O limite preexistente do exemplo no all-targets adicional continua documentado.

R1/R2/R4/R5: splash exclusivo do Well, ID de upgrade `machine_well`, spin industrial
anterior ao débito e guarda de licença por todos os campos corrigidos. A guarda
recusa negações/conflitos e só excepciona o aviso conhecido do próprio GoldWave.

`HORIZON_AUDIO_TEST_MIXER=1` mantém o mixer/decoder reais ativos sem abrir hardware,
com canais, PCM, cursor, ganho, pitch e histórico stop/destroy no status. Esse é um
modo diagnóstico opt-in; `HORIZON_NO_AUDIO=1` permanece nos testes. A prova pelo
dono NTM passou 68 verificações. O handle nuclear agora para e libera ao fechar ou
trocar o mundo. A porta produz 2 traces de resolução, mas 1 canal da janela e 1
transição por interação; o teste distingue essas cardinalidades. LAN com dois
clientes continua na onda 7.

Release de aceitação: código NTM `4771ab2b`, motor `f22df7b81`;
binário SHA-256 `aac79a3bec5e21a40eba1139b075abbc3bdb109dbea6a2659b1132201e51bc76`;
pacote SHA-256 `9ab18eacc7e9e1573d18439d3d675191eb9aaaa43a949300bd7dfe36a3cee0aa`.
A nova regressão integral concluiu 19/19 relatórios e 1.022 verificações em
`proofs-review-v4`: 68 da prova ativa, 13 de resolução/decoder e 941 gerais.
Os 17 cenários CLI retornaram 0; dims inclui idle e idle-clean. Todos usam o mesmo
binário; inventory-proxy usa sua fixture oficial, e os outros 18 usam o pacote NTM. Detalhes em [sons-hbm.md](sons-hbm.md).

## RBMK T01 — contrato transacional sobre o save atômico (09/11/2026)

Branch `rbmk-t01` sobre `main` `91e309f5` (save atômico R7c), com `front-rbmk-core` `589ca13a` mergeado em `94f2af00`
(decisão D-a do coordenador); motor `a31497f38`, sem mudança de motor. Ticket T01 do artifact `rbmk-plano`.
Três partes, todas sobre o contrato do R7c (toda obrigação no registro do chunk que a originou; nada dado a um registro
depois que um save o tomou):

| Parte | Como fica |
| --- | --- |
| Transferência jogador↔BE (B) | `ServerHost::player_slots` lê as pilhas do jogador que age (cada uma com o SNBT inteiro) e `commit_player_transfer` publica num passo só: a glue confere todo o lado das máquinas (slots, geração, células), publica o lado do jogador por `native_commit_player` (CAS do inventário inteiro, cursor, mão secundária e attachment; recusa não muda nada) e então as máquinas, cuja publicação não falha depois das conferências; tudo no mesmo callback com `&mut Game` exclusivo. Uma pilha editada mantém os demais componentes. Hosts sem suporte recusam: não há caminho que debite um lado depois do outro. Primeiro usuário: o carregador da pile, cuja barra saía da mão depois do commit (`PileUse::ConsumeRod`); agora sai na mesma transferência. O segundo é a barra RBMK na mão (`RBMKRod.onBlockActivated`: copia uma, debita só fora do criativo, `upgradePlug`). |
| Recibos de entidades (C) | `WorldEffect::Entities` (tipo, posição exata, compound SNBT) segue o caminho dos itens: com o chunk de destino carregado, `apply_entities` invoca as entidades por `native_summon`; senão ficam devidas (`ChunkLedger::owe_entities`) no registro de um dono carregado e saem quando o destino carrega. O id do efeito (`serial`,`done`) impede o segundo spawn num replay. Host sem entidades recusa: o lote espera e o dono congela, nada se perde. O tipo de detrito do RBMK é do T20 (D9). |
| Grupo acoplado com RNG/fila (A) | Cada **reator** é um grupo de `steps()`: o componente conexo das colunas de uma camada (dimensão, Y da base) que os fluxos e o calor ligam (segunda rodada, P0-1, abaixo). Suas colunas (BEs) e o estado do componente no KV (`java.util.Random` por D3 e a fila de fluxos para o próximo passo) avançam juntos no estado especulativo e voltam juntos com `self.ready = before` se o commit é recusado. Uma coluna do componente num chunk desconhecido, ou mostrada pelo mundo e ainda não adotada, ou uma célula que os fluxos dele leem desconhecida, congela só aquele reator (D4: desconhecido nunca é ar). Doubles guardados pelos bits (NaN e −0,0 inclusos). A tag da barra (`yield`/`xenon`/`core`/`hull`) vai no `custom_data` da própria pilha do slot, com os outros campos preservados e os doubles sem perda (P2-1). |

**Provisório (D-b), assumido por outros tickets.** O passo ligado é o mínimo para provar o contrato: oito colunas
(`rbmk_rod`, `_mod`, `_reasim`, `_reasim_mod`, `blank`, `moderator`, `reflector`, `absorber`) viraram `Kind::Rbmk`; dials
padrão, altura 4, colunas sem tampa (o Java coloca sem tampa: `RBMKBase.getDirModified` → `DIR_NO_LID`), oclusão "todo
bloco não ar é opaco", `rad`/`effects` ignorados, `meltdowns` só registrados no log, menu genérico do slot sem a regra de
casco quente, valores de tela neutros. T02 (altura/tampas/portas), T03 (codec da pilha, tooltip, hazards), T04 (dials,
oclusão HBM, radiação, escalonador), T05 (slots por contexto), T16 (fusão) e T19 (visual) substituem cada item; ver
`docs/rbmk-core.md`.

**Limitação conhecida (D-c): o estado dos reatores mora no KV** (decisão do coordenador mantida na segunda rodada). Ele
não guarda item nenhum, então não fere o princípio das obrigações. Mas o KV é gravado no fim do save, depois das regiões:
numa morte entre as regiões e o KV, a RNG e a fila voltam ao save anterior enquanto as colunas estão como as regiões as
deixaram (um passo de fluxo repetido ou perdido; nenhum recurso duplicado ou perdido, o que as provas abaixo conferem).
O Java nem persiste a fila: ela vive só em memória, na lista `StreamWorld.streams` de cada mundo
(`J/handler/neutron/NeutronNodeWorld.java:47`; `addStream` em `:60-62`, percurso em `:54-58`; o mapa de mundos
`streamWorlds` em `:13`; sem NBT), e a RNG é o `world.rand`, ressemeado
a cada carga.

**Gate obrigatório (P2-4): T09 (outgasser) e T16 (fusão) não começam com o estado no KV.** Neles o fluxo produz itens, e
o mesmo resíduo (um passo repetido ou perdido) vira duplicação ou perda de item. Antes deles, o estado do componente
tem de ir para o registro do chunk-casa do reator, gravado com as colunas.

**Validação (09/11/2026).** Código de produção no commit `db46103b`; binário release `horizon-client` SHA-256
`dcafa5147326778857db84ad1bac226fefe360d610a417565c56670d312a7290` (motor `a31497f38`, fontes do stage idênticos ao
commit), pacote `ntm.hmod` SHA-256 `9ab18eacc7e9e1573d18439d3d675191eb9aaaa43a949300bd7dfe36a3cee0aa` (conteúdo
inalterado). NTM em debug: 502 testes da lib e 47 de integração (as paridades RBMK do oráculo inclusas, a física não
mudou); o verificador do save (`save_model`) em release, 11/11; Clippy `-D warnings` de `ntm-core` com todos os alvos.

Testes de unidade do T01: transferência na pile (recusa do jogador, recusa da máquina, mão diferente, sucesso, carregador
cheio) e na coluna RBMK (criativo copia sem debitar, última barra esvazia a mão, recusa sem efeito nos dois lados);
camada que passa e recusa junto; camada que espera inteira por coluna desconhecida, coluna mostrada e não adotada, ou
célula ao alcance desconhecida; mundo reaberto que segue **bit a bit** igual ao que nunca fechou (colunas, barras, ordem,
RNG e fila); KV que falha a escrita e regrava depois; recibo de entidades aplicado uma vez, replay sem segunda cópia e
host sem entidades esperando sem perda; codecs de coluna/camada por bits e da tag da barra preservando campos alheios.
Dez mutações: oito do código (fila ou RNG não guardadas, coluna desconhecida tomada como removida, célula desconhecida
como ar, coluna mostrada e não adotada como removida, tag não regravada, doubles estreitados, camada fora do KV) e uma do
host de teste (publicar o jogador antes de conferir as máquinas) foram pegas; a décima (tirar o `staged.valid=false`
depois de `rbmk_tick` recusar) é equivalente, porque o retorno antecipado já impede o passo.

**Prova em jogo** (`tools/test-game/run-rbmk-t01-proof.py` + `native_rbmk_t01.py`, molde do `run-crash-proof.py` do R7c,
mesmos veredictos). Mundo NTM: uma camada sobre a borda x=48 (dois canais em cada chunk, uma fonte ReaSim de Po-210/Be
que consome a RNG da camada a cada tick e um refletor) e uma segunda camada no Nether, estacionado durante o ato. O ato
põe 4 barras pela mão (sobrevivência) nos 4 canais vazios e pede um save, morto em cada fronteira. Controle vanilla: barris
nas mesmas posições, a barra de cada vez do jogador para o barril por comando, antes do save. Contagem por item, no
disco, depois de cada uma das duas reaberturas, contra o controle no mesmo corte; KV sem item em nenhum mundo.

| Corte | Barras no disco e nas duas reaberturas (NTM = controle) | Leitura |
| --- | --- | --- |
| `none`, `kill`, `save_begin`, `region_record:1..3`, `chunks_written`, `entities_written`, `level_saved`, `level_dat_written`, `metadata_saved`, `players_saved`, `world_saved` | 9 = 9 | nada perdido nem copiado; `kill` volta exatamente ao último save nos dois; `region_record:2` gravou chunks diferentes nos dois mundos (a camada do mod suja mais chunks) e foi pareado, como no R7c, com o `save_begin` do controle, que gravou os mesmos |
| `parked_saved` | 13 = 13 | resíduo vanilla: o arquivo do jogador ficou atrás das regiões, as 4 barras movidas aparecem na mão e nos canais/barris |
| `live_chunk:2,0`, `live_chunk:3,0` | 11 = 11 | o mesmo resíduo, só nas 2 barras do chunk cortado |
| `kv_committed` | 5 = 5 | resíduo vanilla: a pausa grava o arquivo do jogador à frente das regiões retidas; as 4 barras somem da mão e dos canais/barris |

Duas execuções completas e independentes, 17/17 cada, todas com o mesmo binário e pacote; KV sem item nos dois mundos em
todas as aberturas. Os 13 primeiros pontos passaram na primeira rodada; `parked_saved`, `kv_committed` e os dois
`live_chunk` foram refeitos depois de corrigir o **critério**, não o mod: o `verify` exigia "≤ 9" e o veredicto exigia
"exatamente o último save" também no `kv_committed`; com o jogador como um dos lados da transferência, o vanilla copia ou
perde nesses cortes, e a comparação certa é com o controle no mesmo corte (o R7c já registrou os dois resíduos). Em todos
os cortes das duas rodadas, o NTM contou o mesmo que o controle. Relatórios em `.agent-tmp/rbmk-t01/evidence/`; mundos
apagados.

**Achados do controle (motor, a confirmar):** (1) `item replace block` num barril/baú não fez o save gravar o chunk: num
save limpo o controle perdeu as 4 barras até o ato mudar também um bloco de cada chunk; o roteiro agora faz isso nos dois
mundos. (2) Em sobrevivência, uma barra Po-210/Be na mão mata o jogador em segundos (radiação do NTM, nos dois mundos): o
roteiro fica em criativo, salvo nas 4 transferências.

**Não provado em jogo (pendências do T01):** os recibos de entidades e os `drops`/cursor da transferência só têm prova de
unidade: nenhum caminho do jogo os produz antes dos detritos (T20/T16) e do retorno da casca (T14), que farão a prova em
jogo deles. A transferência por menus e funis segue o caminho existente do motor (T05).

### RBMK T01 — segunda rodada: correções da revisão independente

A revisão (`revisao-rbmk-t01`) reprovou a primeira entrega por P0-1. O que mudou, item por item:

| Item | Como fica |
| --- | --- |
| P0-1: grupo = reator | A camada lembra cada coluna que registrou (`Member`: número de registro, bloco, componente) e guarda um estado por componente (`CompState`: RNG e fila), sob a célula da sua primeira coluna. Duas colunas se ligam quando uma é canal e a outra está numa célula que os fluxos dela leem (`Reactor::ray_cells`: os quatro raios cardinais, ou as quatro rotações ReaSim de 9° com seus oito raios de 45°, incluindo a célula do `hits` final), ou quando são vizinhas cardinais (`moveHeat`). Os componentes são achados por busca a partir das colunas presentes, limitada ao alcance dos raios: uma coluna fora de todo alcance é outro reator, não congela o primeiro e não faz o passo ler o espaço entre eles. A oclusão provisória lê só as células dos raios. |
| Fusão e divisão (D1/D3) | O Java não tem componentes: um `world.rand` e uma lista de fluxos por mundo (`StreamWorld.streams`, `NeutronNodeWorld.java:47`; `addStream` em `:60-62` na ordem de atualização dos TEs, percorrida em `:54-58`). Com a RNG por simulação de D3: na fusão, fica a RNG do componente cuja primeira coluna foi registrada antes, e os fluxos seguem a ordem de registro das origens (a ordem da lista Java); na divisão, a RNG fica com a parte que tem a primeira coluna sobrevivente do pai, e cada outra parte recebe uma semente derivada do estado do pai e do próprio lugar, sem sortear nada da RNG do pai; cada fluxo vai com a sua origem. Tudo determinístico e igual numa reabertura (teste `reactors_merge_and_split_deterministically`). |
| P1-1 | Uma coluna removida no passo sai do componente; a trocada no lugar por outra é registrada de novo quando adotada. Antes, a troca travava a camada para sempre. |
| P1-2 | A transferência ganhou `SlotCopy`: o slot da máquina recebe a pilha do jogador como ela estava (`ItemStack.copy()`), com todos os componentes, no canal RBMK e no carregador da pile (`BlockPileDevice.java:98`, também `copy()`). **Para o T03:** no Java, uma barra renomeada tem `stackTagCompound` (`{display:…}`), então `getDouble("yield")` devolve 0 (`ItemRBMKRod.java:579-585`) e a barra fica morta. O codec provisório só lê `custom_data`, então a barra renomeada aqui funciona: o T03 tem de reproduzir esse bug. |
| P2-1 | Os quatro doubles da barra passam pela pilha sem perda: os não finitos como o Java os escreve (`NaNd`, `Infinityd`, `-Infinityd`; o NaN canônico, como `writeDouble`), lidos de volta pelo núcleo e convertidos de volta em double pela glue (o SNBT do motor, como o do Java, lê essas palavras como texto). O `canonical` do núcleo escreve ±∞ como o motor. |
| P2-2 | O host de teste espelha a glue (sem id guardado no spawn direto; entidades devidas no registro do dono; obrigações nos registros dos chunks, entregues na reabertura), e o teste de replay reabre de um save antes e de um depois. Tipo que o motor não sabe invocar (`summonable`) é recusado: o efeito espera, sem gastar. Gate do T20: o tipo de detrito tem de estar registrado como invocável. |
| P2-3 | Os reatores ganharam uma chave própria no mesmo KV (`rbmk`, gravada no mesmo save): um reator que passa reescreve só ela, e a chave `world` só é reescrita quando os selos dela mudam. Medido em release (`keep_world_cost`): reator 15×15 com 900 fluxos, chave `rbmk` de 111 KB, **0,156 ms por tick com 0, 5 mil e 50 mil gerações** (antes: 0,21/1,06/7,4 ms). |
| P3 | Fusão registrada no log uma vez por coluna e estado; ar decidido pelo id (`air`, `cave_air`, `void_air`). Adaptações registradas no código: a barra pode vir da mão secundária (1.7.10 não tem), e a inserção consome o clique onde o Java devolve `false` (cairia no `ItemRBMKRod`, que não usa bloco). |
| Cursor (achado da prova C) | Uma transferência que punha uma pilha no cursor sem menu aberto a perdia: o motor só guarda cursor de um menu aberto (o vanilla o devolve ao inventário ao fechar o menu). **Toda edição de cursor é recusada até o T05** (`PlayerTransfer::publishable`, no núcleo, usada pela glue e pelo host de teste; teste `a_cursor_edit_is_never_publishable_before_t05`), sem depender de tela: a do `Game` do servidor é sempre nenhuma, a criativa do cliente zera o cursor ao fechar e a de um convidado de LAN é a do anfitrião. Gate do T05 (menus por contexto) e do T14 (desmontagem com cursor): trazer o menu aberto do jogador que age como contexto antes de liberar o cursor. |
| Pendência C | Ganchos de teste só com `NTM_TEST_HOOKS=1` (como os failpoints; sem a variável o comando nem é registrado): `ntmtest eject` põe a barra de um canal para fora como entidade item pelo `WorldEffect::Entities` do passo do reator, e `ntmtest take` a leva ao cursor do jogador ou a solta da mão, numa transferência. |

**Candidato a upstream (motor, não corrigido aqui):** `item replace block` num contêiner não marca o chunk como alterado:
num save limpo, o controle vanilla da prova perdia as barras postas nos barris por esse comando até o ato mudar também um
bloco de cada chunk. O roteiro faz isso nos dois mundos. Corrigir no Horizon (marcar o chunk/BE como sujo ao editar um
slot por comando) e propor ao upstream.

**Validação da segunda rodada (09/11/2026).** Binário release `horizon-client` SHA-256
`e51afa8bd3ad41afe86dec43930ba4d21661c9d98c2bb0dfac555861b0fdc472`, construído do commit `120e2f30` (fontes do stage
idênticos aos commits, núcleo e glue); pacote `ntm.hmod` `9ab18eac…` (conteúdo inalterado). Os commits seguintes só
mudam roteiros de prova e docs. NTM em debug: 511 testes da lib (2 ignorados: o doc-test já ignorado e a medição
`keep_world_cost`, que roda em release) e 47 de integração; `save_model` em release 11/11; Clippy `-D warnings` do
`ntm-core` (todos os alvos) e da glue (`horizon-client` e `horizon-client-lib` no stage, release): 0 avisos.

Mutações da segunda rodada (`evidence/mutations-r2.log`): dez, todas pegas: coluna desconhecida tomada como removida,
célula desconhecida como ar, coluna mostrada e não adotada como removida, coluna removida no passo mantida (P1-1), sem os
elos inversos dos raios, herança da RNG perdida na divisão, fluxos da fusão sem a ordem de registro, oclusão truncada,
double não finito descartado (P2-1) e um grupo por camada (P0-1). As duas sobreviventes da primeira passada ganharam
testes próprios (coluna ligada só pelo calor; `settle` puro com registros intercalados; obstáculo entre fonte e canal;
igualdade bit a bit das células dos raios contra todos os obstáculos em 60 passos).

Em jogo, no binário acima:

| Prova | Resultado |
| --- | --- |
| Sondas (`--probe`) | P0-1: com uma coluna em (640, 101, 640), carregada por forceload e depois descarregada, o calor da fonte segue mudando (carregada: 28,35204 → 28,34481; descarregada: 28,33343 → 28,32636). P1-1: canal trocado no lugar por `/setblock` de um blank, e depois ar e um canal de novo: o reator segue. P1-2: barra com `custom_name`, `lore` e um campo alheio no `custom_data`, posta à mão: tudo no canal e no item que a coluna derrama ao ser quebrada. 8/8 checagens. |
| Cenário C (`--scenario c`), duas execuções | `kill`, `kv_committed`, `players_saved`, `live_chunk:2,0` e `live_chunk:3,0`: 5/5 em cada, NTM igual ao controle item a item nas duas reaberturas (3 = 3, ou 2 = 2 nos `live_chunk`, resíduo vanilla do chunk de entidades não gravado). A barra sai como entidade pelo recibo do passo do reator, a edição de cursor é recusada (com e sem menu aberto) e a barra fica no canal, e o drop sai numa transferência. |
| Cenário principal, duas execuções | 17/17 em cada, NTM igual ao controle item a item em todos os cortes, com o mesmo quadro da primeira rodada: 9 = 9 em 13 cortes (`kill` = último save exato; `region_record:2` pareado com o `save_begin` do controle, que gravou os mesmos chunks); `parked_saved` 13 = 13, `live_chunk:*` 11 = 11 e `kv_committed` 5 = 5, os resíduos vanilla do arquivo do jogador. KV sem item em todas as aberturas. |

Relatórios em `.agent-tmp/rbmk-t01/evidence/` (`final-*-report.json`, `final-*.log`, `mutations-r2.log`,
`clippy-glue.log`); mundos apagados. Mudanças de critério do runner, ambas a favor do controle e registradas: o `exact`
só no `kill` (primeira rodada) e, no cenário C, uma cópia do vanilla num corte não é meta para o mod (a perda é comparada
com `max(perda do controle, 0)`, item a item também).

**Limitações conhecidas (revisão da segunda rodada, P3):**

- **P3-4, divisão com uma parte desconhecida.** Se uma quebra divide um reator enquanto uma parte dele está num chunk
  descarregado, o grupo do tick ainda é o reator inteiro e tem um componente retido: o passo é recusado e a parte
  carregada espera a outra carregar (o Java seguiria com a parte carregada). Sem perda: a barra do canal quebrado cai pelo
  motor, e a reabertura é determinística. Consequência de D4 (operação acoplada indisponível congela); refinar no T04
  (dar o passo só aos componentes não retidos).
- **P3-5, `/setblock` sobre um canal abastecido apaga a barra.** É do motor e vale para toda máquina NTM: como o vanilla
  moderno (um barril com itens trocado por `/setblock` não derrama nada), a troca por comando não derrama o inventário,
  enquanto o Java derrama em qualquer `breakBlock` (`J/blocks/BlockDummyable.java:312-326`). Só acontece por comando.
  Desvio nominal; tarefa geral de motor (derramar o inventário do BE trocado), fora do RBMK.
- **P3-9, ganchos de teste no binário de release.** `ntmtest` e `rbmk_ejects` estão no release, atrás da variável
  `NTM_TEST_HOOKS` (sem ela o comando nem é registrado) e de permissão de op nível 2, como os failpoints do save. A
  variável nunca vai no lançador nem no pacote instalado; se um dia isso mudar, trocar por uma feature de compilação.

**Pendências que ficam do T01:** (1) o cursor: toda edição de cursor é recusada até o T05 trazer o menu do jogador como
contexto; (2) o tipo de detrito do T20 precisa estar registrado como invocável; (3) D-c é gate de T09 e T16;
(4) o bug Java da barra renomeada (`{display}` → `yield` 0) é do T03; (5) `item replace block` sem marcar o chunk, no
motor, candidato a upstream.

## RBMK T02 — altura, dummies, tampas e portas (09/11/2026)

Branch `rbmk-t02` (a partir de `rbmk-t01` @ `0e8eda47`) e, no motor, `rbmk-t02-engine` (a partir de `ntm-current` @
`a31497f38`). O binário das provas sai de `tools/build-native.sh` com `NEXT_REV=<commit de rbmk-t02-engine>` e
`HORIZON_NEXT` apontando para essa worktree: o pin de `native/ntm/compatibility.json` continua o da `ntm-current` até o
motor ser integrado (o coordenador funde a branch do motor e move o pin).

### O que mudou

| Parte | Comportamento | Fonte Java |
| --- | --- | --- |
| Coluna no motor | Multibloco `column` (1×H, núcleo embaixo), H = 2..16. A altura de uma coluna em pé é lida do mundo: o núcleo e a sequência de `ntm:dummy_rbmk` logo acima (máx. 16). Núcleo a partir de qualquer célula, quebra inteira e contorno usam essa altura; a colocação usa a altura que o mod deu (`set_column_height`) e checa obstrução nela antes de colocar | `BlockDummyable.findCore`, `fillSpace`, `checkRequirement`, `RBMKBase.getDimensions(world)` |
| Dummy próprio | `ntm:dummy_rbmk` (mesma dureza do núcleo); nenhum dummy de outra máquina entra numa sequência de coluna | No Java o dummy é o próprio bloco da coluna |
| 18 colunas adotadas | As dez que eram `Inert` (5 controles, caldeira, outgasser, storage, cooler, aquecedor) viraram `Kind::Rbmk` com `Layout::Native`: tampa, slots do tamanho Java (outgasser 2, storage 12, aquecedor 1) e tamanho da tela por bloco. Só as oito do T01 entram no passo; os slots que o motor já guardava são a verdade da primeira sincronização | `TileEntityRBMKSlottedBase` |
| Dials | As 25 regras do Java guardadas no estado do mundo (`rbmk_rules`, uma vez por save), criadas no início do servidor como `createDials` (padrões salvo `dialSaveDials`; o cache fica com os valores antigos até um refresh, D2). `/ntmrbmk dial [regra [valor]]` (op 2) faz o que `/gamerule` + `RBMKDials.refresh` fazem; só as 25 regras existentes. O passo do reator passa a ler o cache (antes: padrões) | `RBMKDials.createDials`, `refresh`, `GameRuleHelper.parseInt`, `ModEventHandler.commandEvent` |
| Altura (D5) | O cache (`clamp(parseInt or 4, 2, 16)`) vai para o motor; só colunas colocadas depois mudam. `/ntmrbmk migrate x y z [x2 y2 z2]` (caixa ≤ 32768) refaz as colunas em pé para o dial: cresce em ar, encolhe tirando dummies do topo; recusa célula ocupada ou não carregada e diz qual. Cada coluna é um único `set_blocks` no seu chunk (uma coluna nunca cruza chunk) | Desvio D5 (o Java relê o dial e deixa partes órfãs) |
| Órfãos e quebra sem jogador | Um dummy que some por comando ou explosão leva a coluna inteira: as células abaixo dele até o núcleo vão a ar e o núcleo quebra como bloco, o conteúdo do BE devido no registro do chunk na mesma publicação (`commit_structural` + `RetainedDrop`) e a tampa pelo caminho de remoção; os de cima vão como órfãos. Núcleo removido: os dummies vão como órfãos. Célula do caminho não carregada: fica | `BlockDummyable.breakBlock:301-326`, `RBMKBase.breakBlock:183-195`, `destroyIfOrphan` (com `checkChunksExist`) |
| Tampas | Com ou sem shift, `rbmk_lid`/`rbmk_lid_glass` na mão sobre uma coluna sem tampa (qualquer célula): entra, uma sai da mão (não no criativo) na mesma transferência jogador↔BE, som de colocação de pedra/“step.stone” do vidro a 1,0/0,8. Coluna com tampa ou controle (tampa própria): o menu abre. Chave de fenda com shift (sem shift o menu abre antes, como no Java): a tampa sai como item no topo, devida no registro do chunk da coluna junto com a mudança (`RetainedDrop` com nome próprio por remoção) e a ferramenta gasta na mesma publicação; sem tampa: nada, sem desgaste. Coluna quebrada: a tampa cai pelo caminho de remoção de sempre. Os dois drops saem no topo **como o dial diz agora** (`getColumnHeight`), não como a coluna está. O modelo mostra a tampa (valor 0 da tela: 1 tampa, 2 vidro) | `ItemRBMKLid.onItemUse`, `RBMKBase.openInv`, `onScrew`, `breakBlock` |
| Chave de fenda | `screwdriver` (100 de durabilidade) e `screwdriver_desh` (sem desgaste) saíram do backlog de equipamentos: receitas `ToolRecipes` 139/140, nomes, aba de controle. O export da bigorna ganhou a receita que precisava dela (`AnvilRecipes`:523) | `ItemTooling(SCREWDRIVER, …)` |
| Proxies e automação | Porta de inventário no topo dos canais e do outgasser, em todos os dummies do storage. Automação: storage aceita só barras e entrega tudo; outgasser entrega só o slot 1; canais e aquecedor não mostram slot. A mão não é restrita aqui (T05) | `createNewTileEntity` + `TileEntityProxyCombo`, `getAccessibleSlotsFromSide`, `canInsertItem`, `canExtractItem` |
| Portas de fluido | `persist::column_ports` + `LoaderShape` reproduzem `getPortsNoLoader/CloseLoader/ClassicLoader` (loader a 2 abaixo vence a 1) e `PortDef.make`, com teste. Fluxo pelas portas: T08 | `TileEntityRBMKBase.getPorts*`, `TilePort.PortDef` |

### Pontos de contato com o motor (para reaplicar sobre o upstream novo)

Commit `rbmk-t02-engine`: `feat(multiblocks): columns as tall as each was placed`.

| Arquivo | Função / trecho | O quê |
| --- | --- | --- |
| `crates/horizon-world/src/mod_multiblocks.rs` | `Multiblock.column`, `Column`, `ColumnPorts`, `parse` (chave `column`), `Registry.column_heights` (mantido em `load`), `set_column_height`, `placed`/`placed_in`, `standing`, `Multiblock::with_height`, `rise` (coluna conta a altura máxima), `core_of` (coluna: só na mesma x/z, altura em pé), `inventory_core` → `column_port`, dois testes novos | Toda a lógica |
| `crates/horizon-world/src/level/place.rs` | `place`, bloco "A content mod's multiblock": `let m = &crate::mod_multiblocks::placed(&def);` | Colocação com a altura do mod |
| `crates/horizon-client/src/game/multiblocks.rs` | `standing` (novo), `machine_multiblock` e `multiblock_outline` usam `self.standing(&def, core)` | Quebra inteira e contorno pela altura em pé |

**Candidato a upstream:** o gancho de altura na colocação (`mod_multiblocks::set_column_height`) e a coluna de altura
variável lida do mundo, úteis a qualquer mod com estruturas de altura configurável.

### Desvios registrados

- D5: altura por instância; dial muda só colunas novas; migração explícita. O Java relê o dial global em todo acesso.
- Drop da tampa (chave e quebra) no topo pelo dial atual, como o Java, mesmo que a coluna tenha outra altura.
- Cache de dials: um só, o do servidor. No Java cada `World` tem o seu: a primeira leitura numa dimensão sem cache
  reescreve a regra lida com o padrão (`getGameRule`, `isIteration`) e o `/gamerule` só refaz o cache do mundo de quem
  digitou. Essa peculiaridade por dimensão fica para o runtime de dials do T04.
- `/setblock` trocando no lugar o núcleo de uma coluna por outra coluna: os dummies seguem com o núcleo novo (no Java
  sumiriam como órfãos, pois o dummy é o próprio bloco da coluna).
- Entrada do outgasser por automação recusada até as receitas reais (T09).
- Lado do Java tomado como referência para os sons da tampa: `Blocks.glass.stepSound.func_150496_b()` é `step.stone` e
  o `concrete_smooth` (pedra) `dig.stone`; aqui `minecraft:block.stone.step` e `minecraft:block.stone.place`, volume
  1,0 e tom 0,8.

### Validação (09/11/2026)

Binário release `horizon-client` SHA-256 `d3258e6650ee7f9ed8bde27d1e75011a01f6cf7e7b8c3660341b7b0666cd4b2e`, construído
de `rbmk-t02` @ `c3097105` (os commits seguintes só mudam roteiros de prova e docs) sobre o motor
`rbmk-t02-engine` @ `5b9d6ee175` (`NEXT_REV`); pacote `ntm.hmod` `cc81c47f…`. Evidências em
`.agent-tmp/rbmk-t02/evidence/`.

| Prova | Resultado |
| --- | --- |
| Motor (`horizon-world`, `mod_multiblocks`) | 7/7 testes (dois novos: coluna lida do mundo, portas top/all, altura do mod com clamp; registro carregado: núcleo de qualquer célula, porta só no topo). Clippy `horizon-world` lib+testes: 0 avisos |
| NTM em debug | 521 testes da lib (2 ignorados) e os de integração, verdes; 8 testes novos do T02 (dials como regras Java e reabertura, coluna de qualquer célula e migração com recusa/célula não carregada/caixa grande, órfãos, tampas com mão/criativo/recusa/chave errada/drop pelo dial/reabertura, quebra com tampa, adoção das 18, automação Java, `RetainedDrop` antigo) e o de portas de fluido |
| `save_model` (release) | 11/11 |
| Clippy | `ntm-core` todos os alvos `-D warnings` e a glue (`horizon-client`, `horizon-client-lib`, release): 0 avisos |
| Mutações (`evidence/mutations.log`) | 10/10 pegas: cadeia de órfãos não seguida, drop da tampa com o nome da origem, drop num topo fixo, mão debitada no criativo, storage aceitando qualquer item, motor sem a altura no início, controle aceitando tampa, migração crescendo sobre blocos, toda coluna numa camada, dials sempre zerados no início |
| J em jogo (`--play`, sobrevivência) | 27/27 e, após reabrir, 4/4: alturas 2, 4, 16 e clamp/parse (1→2, 99→16, abc→4) com o item consumido; colunas em pé mantêm a altura quando o dial muda; obstrução recusada com o item na mão; quatro lados de colocação (quatro facings) dão a mesma coluna; quebra pela base, pelo meio e pelo topo leva a coluna inteira e devolve um item; tampa pelo topo (uma da mão), nenhuma segunda, nenhuma num controle; chave de fenda com shift: tampa como item, desgaste 1; sem tampa, nenhum desgaste; tampa de vidro; núcleo trocado por comando leva os dummies; `/ntmrbmk migrate` refaz 2 → 6 e 16 → 6; funil no topo do storage passa barra e segura pedra, no topo do canal segura a barra. Reaberto: dial 4, alturas 6/2/16, tampa de vidro, coluna nova com 4 |
| C em jogo, duas execuções | `kill`, `kv_committed`, `players_saved`, `live_chunk:2,0`: 4/4 em cada, NTM igual ao controle item a item nas duas reaberturas (3 = 3; 2 = 2 nos cortes em que o controle vanilla também perde a última escrita). O ato: uma tampa da mão para A (transferência), a de B tirada com a chave (item devido no registro do chunk), C quebrada com a tampa (item da coluna e a tampa) |

### Pendências herdadas do T02 (decisão do coordenador, 09/11/2026)

| # | Pendência | Ticket | Critério de saída |
| --- | --- | --- | --- |
| 1 | Visual da altura: o modelo é um OBJ fixo de 4 células com a tampa a 4 blocos e os dummies são invisíveis, então uma coluna de 2 ou de 16, certa física e logicamente, desenha 4 | **Ticket próprio de visual, depois do T04** | Colunas de 2, 4 e 16 desenham suas células e a tampa (as duas) no topo, conferidas por captura em jogo; o gerador visual emite os grupos por célula e a tela leva a altura em pé |
| 2 | Colisão da tampa (`getCollisionBoundingBoxFromPool`, +0,25 em cada célula de coluna tampada) e radiação de coluna destampada | T04 | Caixa de colisão igual à do Java com e sem tampa; dose fora de uma coluna destampada/tampada igual ao Java por teste e em jogo |
| 3 | Cache de dials por dimensão do Java (primeira leitura num `World` sem cache reescreve o padrão da regra lida; `/gamerule` refaz só o mundo de quem digita) | T04 | Teste com dials mudados lidos na Nether e na Superfície reproduzindo a sequência do Java, ou desvio aprovado e registrado |
| 4 | Fluxo de fluido pelas portas e loaders (as formas já existem em `persist::column_ports`) | T08 | Água/vapor e criogel entrando e saindo pelas posições Java (sem loader, loader a 1 e a 2) em jogo, com contagem antes/depois e após queda |
| 5 | Entrada do outgasser por automação (receitas reais) | T09 | Funil insere só insumos com receita no slot 0 e retira só do 1, em jogo |
| 5b | Fila do storage e guindaste | T11 | Conforme o T11 do plano |
| 5c | Física das dez colunas adotadas (controles, caldeira, cooler, aquecedor, outgasser, storage no passo) | T08/T09/T11 | Cada coluna no passo do reator com paridade de oráculo |
| 6 | ~~Dureza das colunas e do dummy~~ | **Feito** (`837a5fb9`) | Teste `test_rbmk_strength` contra o `RBMKBase` (3/30 → 3/18) |
| 7 | Rebase da `rbmk-t02-engine` sobre o upstream novo | Quando o agente do Horizon fechar a atualização | Commit reaplicado sobre a `ntm-current` nova, testes do motor, glue e provas J/C repetidos no binário novo |

Desvios registrados (não pendências): D5; drop da tampa pelo dial (como o Java); `/setblock` trocando o núcleo por
outra coluna mantém os dummies; o pin de `compatibility.json` segue a `ntm-current` até o motor ser integrado.

### Correções da revisão independente (`revisao-rbmk-t02`)

| Achado | Correção |
| --- | --- |
| P1-1 | `rbmk_columns::rbmk_breaks`: o dummy que some por comando ou explosão desce a corrente até o núcleo (`BlockDummyable.breakBlock`). O núcleo e os dummies abaixo saem numa única `commit_structural` do chunk da coluna, com o conteúdo do BE como `RetainedDrop` de nome próprio; a tampa cai pelo caminho de remoção, no topo pelo dial. Sem item da coluna (ninguém colheu). O teste que fixava o comportamento antigo foi trocado (`a_dummy_taken_away_takes_the_whole_column_and_drops_its_contents_once`), e a J e o C ganharam um canal com barra e tampa cujo dummy do meio sai por `/setblock`. Explosões: com a resistência Java (18), TNT e creeper não quebram células de coluna, aqui e no Java; uma explosão que quebre um dummy passa pelo mesmo caminho (as bombas são do T16) |
| P1-1 (achado na reprova J) | A migração encolhendo uma coluna tira dummies do topo: essas escritas são do mod (`set_machine_blocks` com as células como suas, o escopo de blocos próprios do host nativo), não "dummy tirado", senão a corrente do P1-1 quebraria a coluna (a J mostrou 16 → 6 deixando nada). Teste `a_column_is_found_from_any_cell_and_rebuilt_only_on_demand` com as escritas devolvidas como o jogo devolve; a mutação (voltar a `set_blocks`) é pega |
| P2-1 | A J conta a tampa tirada com a chave em entidades + inventário (a tampa sai sem atraso de coleta, como no Java, e o jogador pode pegá-la). Reexecutada inteira com o pacote atual |
| P2-2 | **Escolha: confirmação explícita.** `/ntmrbmk dial dialColumnHeight <v>` cuja altura efetiva seja diferente de 4 é recusado com a explicação ("o modelo das colunas ainda desenha 4 células até o ticket T02-V") e só vale com `confirmar` no fim; mesmo assim responde com o aviso. Motivo: mantém o dial do Java disponível (2..16, D5) para quem precisa, sem que alguém o mude por engano e veja um reator enganoso; limitar a 4 tiraria uma regra do Java até o T02-V |
| P3-1 | **Custo conhecido, sem mudança.** A velocidade já é a do `new EntityItem` do Java (±0,1 horizontal, 0,2 para cima: `dispenser::new_item_motion`). O voo vem de a tampa nascer dentro da célula do topo e ser empurrada para fora dela, como no 1.7.10: `func_145771_j` empurra qualquer entidade dentro de uma caixa de colisão, e toda coluna tem colisão inteira (`RBMKBase.getCollisionBoundingBoxFromPool:127-148`), canais inclusive. Só a direção difere: no Java a primeira face livre testada é −x; o motor segue a sua ordem. Igualar a direção exigiria a física de entidade do motor; fica registrado |
| P3-2 | `core_of` de uma célula de coluna desce pela sequência do próprio dummy (`column_core`); a busca dos outros multiblocos volta à altura deles (`rise` sem colunas) |
| P3-3 | `Registry.column_dummies` (montado no `load`) no lugar de percorrer as definições a cada consulta de funil |
| P3-4 | Ver P1-1 |
| P3-5 | Evidências refeitas com o pacote do HEAD (dureza 3/18) |

Pontos de contato com o motor: o segundo commit da `rbmk-t02-engine` (`perf(multiblocks): columns find their core down
their own run`) mexe só em `mod_multiblocks.rs` (`Registry.column_dummies`/`column_rise`, `load`, `Multiblock::rise`,
`core_of` → `column_core`, `inventory_core`, `column_port`, testes).

O T03 acrescentou um terceiro commit à `rbmk-t02-engine` (`93359657aa`, `feat(mods): the host's crafting grid asks
prepare_item_craft too`): ver a tabela única "Contatos de motor do RBMK" no fim do T03.

### Ressalvas da re-revisão (P3, registradas)

| Achado | Registro |
| --- | --- |
| P3-6 | A fila `rbmk_breaks` (dummy tirado → corrente até o núcleo) vive só em memória. Se o chunk descarregar ou o save cair dentro do mesmo tick em que o dummy saiu e antes de a fila rodar, a coluna fica mais baixa, sem perda nem cópia de item (o BE e o conteúdo ficam no núcleo). Ao reabrir, a coluna mais baixa é uma coluna válida; não há retomada da corrente |
| P3-7 | `/setblock` sobre o **núcleo** segue o `/setblock` moderno do motor, que apaga o BE sem derramar (como um baú no vanilla atual); no Java o `breakBlock` derramaria o inventário. A tampa cai mesmo assim (caminho de remoção do mod). Quebra pelo jogador, explosão e dummy por comando derramam |
| P3-8 | Numa explosão, o Java dropa um item da coluna por célula explodida, com chance 1/tamanho da explosão (`Block.dropBlockAsItemWithChance`), dummies inclusive (o dummy é o próprio bloco da coluna). O port não dropa nenhum: o dummy é um bloco sem item e o núcleo quebrado pela corrente não dá o item. Desvio que ficou de fora antes; com a resistência Java (18) só explosões fortes chegam lá (T16) |

O cenário C passa a contar também o item da coluna quebrada pelo jogador (`ntm:rbmk_blank`) na próxima rodada de
provas do T02 (no rebase sobre o motor novo).


### Validação das correções (10/11/2026)

Binário release `horizon-client` SHA-256 `abbd98a934ddf5fbb3a278746ceb87e295c2c101e836a181f57bd3a87433d868`, de
`rbmk-t02` @ `49842d28` sobre `rbmk-t02-engine` @ `06bf7ffe5c` (`NEXT_REV`); pacote `ntm.hmod`
`739cc99c6b418c36a2f15c4976de07b1f6a18f06eda918735656f5ef487c0094` (o do HEAD, dureza 3/18). Evidências em
`.agent-tmp/rbmk-t02/evidence/r2/` e `evidence/`.

| Prova | Resultado |
| --- | --- |
| Motor | 7/7 testes de `mod_multiblocks` (um novo: a busca dos outros multiblocos não cresce com coluna carregada) |
| NTM debug | lib 522 (2 ignorados) e 47 de integração (todos os alvos menos `save_model`); 9 testes do T02 |
| `save_model` (release) | 11/11 |
| Clippy | `ntm-core` todos os alvos `-D warnings`, glue (`horizon-client`, `horizon-client-lib`, release, no stage de `06bf7ffe5c`) e `horizon-world` lib+testes: 0 avisos |
| Mutações novas | sem a corrente para baixo (P1-1): 2 testes falham; migração com `set_blocks` em vez de escritas próprias: o teste da migração falha |
| J em jogo | 29/29 e, após reabrir, 4/4; inclui o canal com barra e tampa cujo dummy do meio sai por `/setblock` (coluna inteira some, barra +1, tampa +1, item da coluna 0) e a tampa da chave contada em chão + inventário |
| C em jogo, duas execuções | `kill`, `kv_committed`, `players_saved`, `live_chunk:2,0`: 4/4 em cada, NTM igual ao controle item a item nas duas reaberturas: 5 = 5 (4 tampas, 1 barra) em `kill` e `players_saved`; 4 = 4 em `kv_committed` e 2 = 2 em `live_chunk:2,0` (a última escrita que o vanilla também perde). O ato agora inclui o canal D com barra e tampa cujo dummy do meio sai por `/setblock` (no controle, o barril quebrado com `destroy`) |

## RBMK T03 — barras, xenônio e hazards dinâmicos (pronto para revisão, 10/11/2026)

Branch `rbmk-t03` (a partir de `rbmk-t02` @ `5582ece5`). Escopo do plano: codec de stack, queima, nove curvas e cinco
depleções, coeficiente térmico, core/hull/transfer, tooltips e radiação/HOT. Aceitação: U cada barra e função em
entradas nulas/limites/NaN admitidos, yield/poison e sequência de calor, defaults NBT; J retirar/reinserir barra entre
inventários e reabrir, observar hazard e tooltip do mesmo stack; C yield só reduz conforme o Java, componentes alheios
preservados, retirar não zera calor/poison. Depende de T00/T01, D2/D3/D6.

| Item | Fonte Java | Situação de partida |
| --- | --- | --- |
| Codec definitivo da pilha: `stackTagCompound` existe quando a pilha tem **qualquer** dado do 1.7.10 (nome, lore, encantamentos, `RepairCost`, `custom_data`), não só `custom_data`; sem tag, `setNBTDefaults` (yield = máximo, core/hull 20, xenônio ausente → 0) na primeira leitura ou escrita; com tag e chave ausente, `getDouble` → 0 | `ItemRBMKRod.getDouble/setDouble/setNBTDefaults` (571-598) | Provisório do T01 (`persist::rod_stack`) só olha `custom_data` |
| **Bug da barra renomeada:** um nome (ou lore, ou encantamento) cria o `stackTagCompound` sem `yield`, que lê 0: a barra fica morta | `getDouble` (579-585) | Hoje funciona (desvio a corrigir) |
| `onCreated` (feitio): `setNBTDefaults` no resultado da receita | `onCreated` (600) | Ausente |
| Queima, curvas, depleções, xenônio, coeficiente térmico, `updateHeat`/`provideHeat`, `rectify` | `burn`, `updateHeat`, `provideHeat`, `reactivityFunc`, `reactivityModByEnrichment` | Núcleo puro com paridade de oráculo (`rods.rs`); ligar ao codec definitivo |
| Barra de durabilidade: mostrada quando `1 − enrichment > 0`, valor `1 − enrichment` | `showDurabilityBar`, `getDurabilityForDisplay` (562-569) | Ausente |
| Tooltip dinâmico (as duas variantes, RBMK e DRX/rbmx): nome completo, resfriando (≥ 50 °C), fonte, depleção, xenônio, n/r, função de fluxo e tipo, xenônio gen/burn, calor, difusão, casco, núcleo, fusão, com os arredondamentos Java | `addInformation` (447-516), `getFuncDescription` | Tooltip estático do T00 |
| Hazards dinâmicos: radiação pela depleção (linear ou `1 − e²`) + Xe135 pelo veneno; HOT `int(min(ceil((hull − 100)/10), 60))`; pellets pelos cinco graus e veneno | `HazardModifierRBMKRadiation`, `HazardModifierRBMKHot`, `HazardRegistry` | Taxas estáticas por id (`radiation.rs`) |

**Estado em 10/11/2026 (antes da revisão).** Feito, item por item da tabela acima:

| Item | Como ficou | Prova de unidade |
| --- | --- | --- |
| Codec da pilha | `Stack::tagged` marca o `stackTagCompound` do 1.7.10 (nome, lore, encantamentos, custo de reparo ou `custom_data`); `persist::stack_rod` lê as chaves com os defaults do Java (sem tag: `setNBTDefaults`; com tag e sem a chave: 0). A glue marca `tagged` nos três caminhos (slots de máquina, inventário do jogador lido pelo servidor, cliente) | `a_stack_with_a_tag_but_no_key_reads_a_dead_rod_as_java` |
| Barra renomeada (bug Java) | Uma barra só com nome lê `yield` 0: morta no reator, radiação no alvo, barra de durabilidade cheia | o mesmo teste, `a_rod_goes_towards_its_target…` e o de radiação do servidor |
| `onCreated` | O resultado da receita sai com `setNBTDefaults` (gancho `prepare_item_craft`, injeção `rbmk-rod-crafted`) | prova J |
| Queima/curvas/calor | Já no núcleo (`rods.rs`, paridade de oráculo do T00/T01), agora lendo o codec definitivo | testes do T00/T01 |
| Barra de durabilidade | `tooltip::durability_bar`: mostrada quando `1 − enrichment > 0`, largura e cor como o Forge 1.7.10 as desenha (`round(13 − 13·h)`, cor de `round(255 − 255·h)`); gancho `item_bar` (injeção `rbmk-rod-bar`) | `the_bar_is_forges_for_1_minus_the_enrichment` |
| Tooltip | `tooltip::lines`: as linhas de `addInformation` nas duas variantes, com `Double.toString` e os truncamentos do Java; as chaves de idioma do Java nos dois idiomas | testes de `tooltip.rs` |
| Radiação dinâmica | `rbmk::hazards::radiation_rate`: `level + (target − level)·depleção` (linear ou `1 − e²`), floats e doubles como o Java, `+ xe135·veneno`; o extrator de hazards exporta `target`, `linear`, `hot`, `xe135` e `nugget` de cada `registerRBMK*`. Entra na soma por segundo do servidor e no tooltip | `a_rod_goes_towards_its_target_as_it_depletes_plus_its_xenon`, `carried_pellet_rate_uses_custom_long…` |
| HOT | `rbmk::hazards::hot_level` (`(int) min(ceil((casco − 100)/10), 60)`); a cada tick o servidor acende o jogador pelo maior nível que ele carrega (`setFire`, o maior entre o que resta e o novo, menos a proteção contra fogo), salvo molhado (água ou chuva nos pés ou no topo) ou com um `reacher` no inventário; tooltip `[trait.hot]` em dourado, depois da radiação, como o Java registra | `a_rod_is_hot_from_its_hull`, `a_hot_rod_sets_its_holder_on_fire_unless_they_carry_a_reacher` |

Pendências honestas (fora do T03 ou adaptações):

- **Pellets** (graus pela `metadata` 0–9 e o xenônio): o port não tem subtipos de pellet (32 ids, sem metadata). A
  radiação deles fica a fresca. Vão com o T14 (a desmontagem é o que os produz), que tem de decidir como o grau vive na
  pilha.
- **Digamma da barra DRX (1/3) e cegueira:** o port não tem os sistemas `DIGAMMA` e `BLINDING` de hazard (nenhum item
  os aplica). Pendência do sistema de hazards geral, não do RBMK.
- **`reacher` na radiação** (`BobMathUtil.squirt` da taxa): o sistema de radiação geral do port não o considera; só o
  HOT o considera. Mesma pendência geral.
- **NaN:** um `level` NaN não contamina (como o Java, `level > 0` é falso), mas o tooltip Java ainda escreveria
  `[Radioactive] NaNRAD/s`; o port não mostra linha. Só alcançável com uma tag NaN.
- **Cadência:** a radiação segue a soma por segundo do port (o Java aplica por tick; a dose é a mesma); o HOT é por tick,
  como o Java. O inventário lido inclui a mão secundária, que o 1.7.10 não tem, como a radiação já fazia.
- **Molhado (contato de motor):** visitar a cada tick (`native_visit_player`) um convidado que já está em chamas e
  entra na água trazia o fogo de volta (a água apagava uma vez; depois as leituras davam 600). Sem visita, o fogo
  ficava apagado. A glue agora lê "molhado" (água no bloco dos pés ou chuva) pela posição publicada do jogador, sem
  visitar, e só visita um jogador seco para acendê-lo (`52d7bb62`; antes, `2ae9c805` só somava a água dos pés). A
  prova J checa as duas coisas: em chamas e com a barra, apaga na água. A causa no motor (a troca de estado da visita
  contra a cópia do movimento do convidado) fica para o agente do Horizon: qualquer visita por tick a um convidado em
  chamas pode regravar o fogo.
- **Mobs** segurando barras (`updateLivingInventory`) não pegam fogo nem radiação: o port não tem esse caminho para item
  nenhum.
- **Proteção contra fogo:** resolvida no HOT na revisão (P2-1, `7d9cb26d`: o maior nível da armadura, como o
  1.7.10). O resto do fogo do motor usa o atributo moderno `burning_time`, que soma as peças: lacuna nominal da Onda 4.

**Validação (10/11/2026).** Commits do T03 na `rbmk-t03`: `4341a3c2` (codec e `onCreated`), `199586c5` (tooltip),
`0f0a2e97` (barra de durabilidade), `87c6f96b` (radiação e HOT), `2ae9c805` e `52d7bb62` (HOT e água), `5e79136a`
(cores do tooltip), `bc7fa29c` (componentes da pilha que sai de uma máquina), `431a00af`/`14100a59` (provas),
`35650ea6`/`bf5c19a4` (clippy). No motor, `rbmk-t02-engine`: `93359657aa`, `01e5147ef5`, `00454b508a` (tabela abaixo).
NTM em debug: 530 testes da lib (2 ignorados) e 47 de integração; `save_model` em release 11/11; Clippy `-D warnings`
de `ntm-core` com todos os alvos: 0 avisos; glue (`horizon-client`, `horizon-client-lib`, release, no stage de
`00454b508a`): 0 avisos; `horizon-protocol` lib+testes: 0 avisos; `item::hash` 9/9. Binário final `567a72b9f30bdde7a145d3559efcea41e882262854b037562a51126693f6560c`
(NTM `bf5c19a4`, motor `00454b508a`). As provas J e C rodaram no `82c16afe…` (NTM `bc7fa29c`, motor `01e5147ef5`); o
que veio depois não está no caminho delas: dois ajustes de clippy sem mudança de comportamento (o da glue gera o mesmo
código) e o `00454b508a`, que só toca a retirada do anfitrião local (sem `serving`); a janela das provas é cliente do
próprio servidor integrado e usa `lan_crafting`.

**Prova J (`tools/test-game/run-rbmk-t03-proof.py --play` + `native_rbmk_t03.py`, 10/11/2026).** Binário
`horizon-client` SHA-256 `82c16afe3177663a9aefb2bf6717977d22986fcd9bef542e89bff18d6ddaafbc` (NTM `bc7fa29c`, motor
`rbmk-t02-engine` @ `01e5147ef5`), pacote `ntm.hmod` `4276ef60dd0358297990bac69d9e506c5e4556dd25247edfa3334be9095cdf61`.
Evidências em `.agent-tmp/rbmk-t03/evidence/j/` (JSON das duas aberturas e os três prints de tooltip). **21 de 21
checagens passaram** (16 na primeira abertura, 5 na reabertura):

| Checagem | Resultado |
| --- | --- |
| Barra feita na mesa pelo anfitrião sai com a tag (lida no servidor) e a janela também a recebe | ok (`yield` 1e8, `core`/`hull` 20) |
| Renomeada depois de feita, continua viva | ok (nome e as três chaves) |
| Queima ao lado de uma fonte Po-210/Be: rendimento cai, calor sobe | ok (xenônio fica em 0: com fluxo acima de 25 a queima de xenônio vence a geração, como no Java) |
| Retirada (gota lançada, apanhada) com a tag; parada fora do reator, não queima | ok |
| Radiação carregada = `HazardModifierRBMKRadiation` da tag (mRAD/s) | ok (2 435 = 2 435 mRAD/s; com 40 % de xenônio, 1 400 350) |
| No baú: a mesma tag bit a bit; nenhuma radiação vinda do baú | ok |
| HOT: com `reacher` não pega fogo; na água não pega; seco pega (600 ticks) | ok |
| Já em chamas, apaga na água, sem e com a barra | ok (6 de 6 e 6 de 6 apagadas). Na rodada anterior, com o mesmo código do HOT (binário `cb092d09…`), 1 de 6 leituras deu 600: intermitente, ver "Molhado (contato de motor)" acima |
| Renomeada (tag sem chaves): radiação no alvo, não queima no canal | ok (1 200 000 mRAD/s; `yield` 0, núcleo 20) |
| Depois de reabrir: a barra do baú igual, carregada com a mesma radiação; num canal isolado, rendimento e xenônio iguais, calor segue | ok |

Os prints mostram as linhas nas cores do Java (verde, roxo, azul, amarelo, dourado, vermelho; `[Pyrophoric / Hot]` em
dourado 255,170,0) e os números como o Java os escreve. A primeira rodada achou o defeito de cor (`5e79136a`). Os prints
das barras de 1 200 e 1 400 RAD/s ficaram de fora: na tela de inventário de sobrevivência a radiação mata em um
segundo; as linhas delas são testadas em `tooltip.rs`.

**Prova C (`tools/test-game/run-rbmk-t03-proof.py` + `native_rbmk_t03.py` `prepare/act/verify/again/drain`,
10/11/2026).** Binário `horizon-client` SHA-256 `82c16afe3177663a9aefb2bf6717977d22986fcd9bef542e89bff18d6ddaafbc` (NTM
`bc7fa29c`, motor `01e5147ef5`), pacote `4276ef60…`. Mundo: no chunk (2, 0), um canal P ao lado de uma fonte ReaSim de
Po-210/Be (S) e um canal Q mais longe; as barras de P e Q feitas por comando com um estado (`yield` 6e7, `xenon` 12,5,
`core` 300, `hull` 250), uma chave alheia (`foreign`) e uma lore, postas pela mão. O ato tira a barra de P como gota
lançada (`ntmtest take`) e o save é cortado. Controle vanilla: barris nos mesmos lugares e a mesma pilha invocada como
item; um bloco do chunk (2, 0) muda nos dois mundos (o `item replace` do motor não marca o chunk, achado do T01).
Relatório em `.agent-tmp/rbmk-t03/evidence/c/report.json`; mundos apagados.

| Corte | Barras no disco e nas duas reaberturas (NTM = controle) | Estado de cada barra (NTM) |
| --- | --- | --- |
| `kill` | 3 = 3 | chave alheia, lore, as quatro chaves, `yield` ≤ 6e7, calor não zerado; P, de volta ao canal, nunca sobe |
| `kv_committed` | 3 = 3 | idem |
| `players_saved` | 3 = 3 (a gota no chão nos dois) | idem, a gota com a lore |
| `live_chunk:2,0` | 2 = 2 | resíduo vanilla: o canal (ou o barril) gravado vazio e a gota não, nos dois; a barra restante íntegra |

Veredictos do R7c em todos: nada duplicado, nada pior que o vanilla, armazenamento sem item, as duas reaberturas iguais.
Antes de passar, a prova achou dois defeitos: (1) a gota de uma barra tirada do canal voltava **sem a lore** no
`players_saved`, porque a glue montava a gota só com o `Stack` da ponte (id, quantidade, `custom_data`); corrigido em
`bc7fa29c` (o que sai de um slot de máquina na mesma transferência usa a pilha do próprio slot como modelo, todos os
componentes, como o `ItemStack.copy()`); vale também para a pilha que um slot do jogador recebe de uma máquina (o
cursor do T05). (2) Os critérios do roteiro: o xenônio cai a 0 com fluxo forte (como no Java) e a fonte ReaSim pode
passar segundos sem acertar P; a checagem passou a ser "a tag não volta ao estado de fábrica" e "o rendimento nunca
sobe". Numa rodada, o save do controle vanilla não respondeu ("server thread did not answer within 10 s"); a rodada foi
refeita inteira.

### Revisão do T03 (a1102939): aprovado com ressalvas, correções

Relatório em `artifacts/revisao-rbmk-t03/index.md`. Sem P0 nem P1. Corrigido em commits pequenos:

| Achado | O quê | Commit |
| --- | --- | --- |
| P2-1 | O HOT acende como o `setFire` do 1.7.10: `t − floor(t · nível · 0.15F)` com o **maior** nível de Proteção contra Fogo da armadura (`getMaxEnchantmentLevel` sobre `getLastActiveItems`, que no jogador é só a armadura, conferido no `EntityPlayer` decompilado do 1.7.10), o maior entre o que resta e o novo, gravado com `set_remaining_fire_ticks` (sem o `burning_time` do motor, que soma as peças). Teste: 0, IV (240), II (420); J: duas peças de IV dão 240 ticks | `7d9cb26d`, `c02fa6b3` |
| P2-2 | A linha de radiação da barra sai do nível em float, como o `HazardTypeRadiation`: nada abaixo de `1e-5`, `Math.floor(level·1000)/1000` com o produto em float e `Double.toString`, a linha `Stack:` igual, NaN como `NaN`. Teste com os 999 estados de depleção da UEU, a 0,998 (6.189) e a balefire gasta (`1.2…E8`, sem o teto de `u32`) | `e94e6651` |
| P3-1 | O nome completo em cinza itálico (`§7§o`), o `§7` que o `GuiScreen.renderToolTip` põe em toda linha depois da primeira | `2b762bc3` |
| P3-3 | O doc de `rod_doubles` volta para cima dela | `01474f20` |
| P3-4 | Sem visita, só a água e a chuva **nos pés**; a chuva no topo fica para a visita, que lê a caixa real | `3a2d1abe` |
| P3-5 | A J espera o servidor ter a barra na mão (`data get entity @s SelectedItem`) depois do `select` | `c02fa6b3` |
| P3-6 | O `onCreated` sempre dá os defaults ao `custom_data` do resultado, como o `new NBTTagCompound()` do `setNBTDefaults` (os outros componentes ficam; nenhuma receita do port os faz) | `01474f20` |

Também da J pós-revisão: a água da prova do HOT virou um poço 1×1 cercado no meio do piso (`a9f16771`). A primeira J
depois da revisão leu o fogo aceso na água porque a corrente levava o jogador para fora da borda do piso, a 4 blocos,
até o chão natural 22 blocos abaixo, seco e com o fogo aceso corretamente (a altura dos pés, agora gravada com cada
leitura, mostrou 78 onde o piso fica em 101). Explica também o "1 de 6 = 600" das rodadas anteriores.

**Provas depois da revisão.** Binário `horizon-client` SHA-256
`08072ff65eac49a4d4b3b17c41754254bccc5a7cb9e510ba668eeb8017ccba17` (NTM `c02fa6b3`, motor `00454b508a`), pacote
`4276ef60…`. **J: 22 de 22** (17 + 5), evidências em `.agent-tmp/rbmk-t03/evidence/j-r2/`: as 21 de antes, mais
`fire_protection_takes_the_highest_piece` (duas peças de IV: 240, 240, 240 ticks). Na água, as 8 leituras de cada
etapa apagadas, os pés em 101. O nome sai em cinza itálico, e a radiação, truncada como no Java (`2.135RAD/s` para um
nível de 2,1359…). **C nos cortes `kill`, `players_saved` e `live_chunk:2,0`: 3 de 3**, NTM = controle (3/3/3, 3/3/3
e 2/2/2, este o resíduo vanilla), todos os veredictos do R7c verdadeiros e cada barra com a chave alheia, a lore e a
tag; relatório em `.agent-tmp/rbmk-t03/evidence/c-r2/report.json`. Testes: lib 532 (2 ignorados) e 47 de integração; `save_model` em release
11/11; clippy `-D warnings` de `ntm-core` com todos os alvos e da glue (stage de `00454b508a`): 0 avisos.

Re-revisão (`881663f7`): aprovada sem P0, P1 nem P2. P3-1 corrigido (o limite `1e-5` comparado em double, como o
Java). P3-2 registrado: a ordem de `fire_ticks` é a do `EnchantmentProtection.getFireTimeForEntity` decompilado do
1.7.10, `(float)ticks * (float)nível * 0.15F`; o resultado é o mesmo.

Registrado, sem mudança de código:

- **P3-2, barra de durabilidade:** o Forge 1.7.10 (`RenderItem.renderItemOverlayIntoGUI`, conferido no decompilado)
  desenha três quads: preto 13×2, uma faixa 12×1 na cor `(255−k)/4 << 16 | 0x3F00` e a barra `j1`×1. O gancho
  `item_bar` do motor (API 60) só aceita largura e cor: a faixa escura não sai. Adaptação; contato de motor enviado ao
  agente do Horizon (um campo de cor de faixa na API).
- **P2-3, fogo na água:** o contorno do NTM (`52d7bb62`) basta e não é P1 (decisão da revisão). Contatos de motor
  enviados ao agente do Horizon: o `tick_remaining_fire` aplica o dano antes de a água apagar
  (`crates/horizon-client/src/motion/environment.rs:37-45`; no 1.7.10, o `handleWaterMovement` zera o fogo antes do
  dano) e a regravação do fogo pela visita por tick. Deixa de estar na lista de bloqueio do T03.
- **Proteção contra Fogo fora do HOT:** o resto do fogo do motor continua somando as peças (`burning_time`). Lacuna
  nominal da Onda 4 (regra de armadura), decisão do coordenador; não bloqueia o T03.

### Motor do T03 sobre o pin novo (`rbmk-t03-engine`, 10/11/2026)

Com a `main` em `e4aa8b2c` (pin `6a79054fa9`, a `ntm-current` sobre o upstream `a3bf6531e`), a `rbmk-t03` foi fundida
nela (`87ade1f2`, depois de `836c7bb7` com a `8ecff5d8`) e o motor das provas passou a ser a `rbmk-t03-engine`, a partir
de `6a79054fa9`, com, nesta ordem:

| Commit | Origem | Nota |
| --- | --- | --- |
| `56fbb210a7` | `5b9d6ee175` (T02, altura das colunas) | Aplicado limpo; o pin novo não tinha nada de coluna (`set_column_height`, `column_core`, `column_heights` ausentes em `mod_multiblocks.rs`) |
| `3b96e2d998` | `06bf7ffe5c` (T02, `column_core` e `column_dummies`) | Aplicado limpo |
| `643e359566` | `00454b508a` (T03, o cursor só empilha com os mesmos componentes) | Portado à mão: no upstream o laço do slot 200 está em `game/menu_clicks.rs` e `host_prepared_craft` em `game/inventory_slots.rs`; a formatação do teste de `hash.rs` já vinha no `88d6b88a2d`. **Exceção** decidida pelo coordenador: entra agora porque a J e a C do T03 dependem dele, e vai pela mesma revisão do T03 |

`01e5147ef5` e `93359657aa` já estão no pin novo (`88d6b88a2d`, `6a79054fa9`). A `rbmk-t02-engine` e a `ntm-current`
não foram tocadas. O pin de `native/ntm/compatibility.json` continua `6a79054fa9`: as provas usam `NEXT_REV` na ponta
da `rbmk-t03-engine`, e o pin muda quando o agente do Horizon integrar esses três commits.

**Provas depois dos merges (10/11/2026).** NTM `57144cee` (com a `main` `e4aa8b2c`), motor `rbmk-t03-engine` @
`643e359566`; binário `horizon-client` SHA-256 `fef5a228546a1770774df2c23a74aaf5d6303d28981e7e3f9c3e1a6755206232`,
pacote `ntm.hmod` `1fc434042789510b844d2fcfdf53cf6d25778eb3afe48850d2f9f0c9c90cb956`. **J: 22 de 22**
(`.agent-tmp/rbmk-t03/evidence/j-merge/`). **C nos cortes `kill`, `players_saved` e `live_chunk:2,0`: 3 de 3**
(`evidence/c-merge/report.json`), NTM = controle em todos (2/2/2, 3/3/3, 2/2/2), cada barra com a chave alheia, a lore e a
tag. No motor novo o `kill` fica com 2 nos dois mundos: depois do ato o motor faz um save sozinho, que grava o canal
(ou o barril) vazio antes da gota; é o resíduo vanilla, igual nos dois. Testes na árvore fundida: os 293 do
`tools/content-gen`; lib 534 (2 ignorados) e 118 de integração em 12 binários; `save_model` em release 11/11; clippy do
`ntm-core`, da glue (stage de `643e359566`) e do `horizon-world` (motor): 0 avisos; `mod_multiblocks` 7/7 e
`item::hash` 9/9 na `rbmk-t03-engine`.

### Contatos de motor do RBMK (todos os commits a portar para o motor novo)

Branch `rbmk-t02-engine`, a partir de `ntm-current` @ `a31497f38`. Cada linha é reaplicada sobre o upstream novo
(`a3bf6531e`, branch `upstream-a3bf653`), com os testes do motor e as provas J/C do ticket repetidos no binário novo.
Já portados pelo agente do Horizon (10/11): `01e5147ef5` como `88d6b88a2d` e `93359657aa` como `6a79054fa9` na
`ntm-current` nova (`6a79054fa9`); `00454b508a` veio da re-revisão desse porte e falta portar. Quem fundir a
`rbmk-t02-engine` roda `cargo fmt` (o `native_hooks.rs` já vinha com diferenças de formatação de antes).

| Commit | Ticket | Arquivo | Função / trecho (no motor atual) | No upstream `a3bf6531e` | O quê |
| --- | --- | --- | --- | --- | --- |
| `5b9d6ee175` `feat(multiblocks): columns as tall as each was placed` | T02 | `crates/horizon-world/src/mod_multiblocks.rs` | `Multiblock.column`, `Column`, `ColumnPorts`, `parse` (`column`), `Registry.column_heights`, `set_column_height`, `placed`/`placed_in`, `standing`, `with_height`, `rise`, `core_of`, `inventory_core` → `column_port`, testes | mesmo arquivo | A altura das colunas lida do mundo, colocação na altura do mod, portas `top`/`all` |
| `5b9d6ee175` | T02 | `crates/horizon-world/src/level/place.rs` | `place`, bloco de multibloco de mod: `placed(&def)` | mesmo arquivo | Colocação com a altura do mod |
| `5b9d6ee175` | T02 | `crates/horizon-client/src/game/multiblocks.rs` | `machine_multiblock`, `multiblock_outline` | mesmo arquivo | Volume em pé no contorno e no núcleo |
| `06bf7ffe5c` `perf(multiblocks): columns find their core down their own run` | T02 | `crates/horizon-world/src/mod_multiblocks.rs` | `Registry.column_dummies`/`column_rise`, `load`, `rise`, `core_of` → `column_core`, `inventory_core`, `column_port`, testes | mesmo arquivo | Busca do núcleo descendo a própria coluna |
| `93359657aa` `feat(mods): the host's crafting grid asks prepare_item_craft too` | T03 | `crates/horizon-client/src/game.rs` | nova `host_prepared_craft` (l. 9115, antes de `craft_result`); no clique do slot 200 (`ResultSlot`, l. 10668) o `while let` passa o resultado por ela antes de entregá-lo | `crates/horizon-client/src/game/menu_clicks.rs`, laço do slot 200 (l. 784); a nova função vai junto de `craft_result`, agora em `game/inventory_slots.rs` (l. 324) | O anfitrião do mundo integrado também chama `native_prepare_item_craft` (API 290) ao pegar o resultado (inventário 2x2: `pos` None; mesa: a posição dela), em cada pilha do shift-click; sem resultado do mod, nada é pego; o que não cabe é recusado como antes. Antes só os convidados de LAN e o servidor dedicado chamavam. Usado pelo `onCreated` da barra RBMK |
| `01e5147ef5` `fix(lan): a click agrees only on the same components` | T03 | `crates/horizon-protocol/src/item/hash.rs` (novo `Hashed::matches`, teste `a_claim_matches_only_the_same_components`) e `crates/horizon-client/src/lan/containers.rs` (`hashed_matches`, l. 509) | `hashed_matches` usa `Hashed::matches` com os registros do hub (`SyncedNames`) | mesmo comparador em `lan/containers.rs:630`; `hash.rs` igual | `HashedStack.matches` do vanilla: item, quantidade e componentes. Antes só item e quantidade: uma mudança de componente feita pelo servidor num clique (a tag do `onCreated` da barra, vinda do gancho no resultado) nunca chegava ao cliente que previu o vanilla. Componente que o servidor não sabe hashear conta como diferente (o slot é reenviado). Agente do Horizon avisado (10/11) |
| `00454b508a` `fix(mods): a crafted result joins the cursor only with its components` | T03 (re-revisão do porte) | `crates/horizon-client/src/game.rs` | no laço do slot 200 do anfitrião, o cursor só empilha o resultado com `same_item_same_components` (antes, só o item); doc de `host_prepared_craft`; `cargo fmt` do teste de `01e5147ef5` em `hash.rs` | `game/menu_clicks.rs`, mesmo laço (l. 784) | Um resultado que o mod preparou (com `custom_data`) não se junta a uma pilha sem ele no cursor (`ItemStack.isSameItemSameComponents`). O gancho é perguntado antes de se saber se a retirada cabe, como na grade de um convidado (perguntado a cada mudança): é o resultado preparado, não a retirada; a glue do NTM não tem efeito colateral nele |

Testes dos commits do T03. `93359657aa`: o motor não tem harness de `Game` para cliques de menu, e o despacho da API
de mods (`contract: replace`) só substitui um resultado vazio, então um teste de `dispatch_prepare_item_craft` não
exercita o caminho que o NTM usa (a injeção no início de `native_prepare_item_craft`). Esse commit vale para o modo local
(janela sem `serving`); no jogo das provas a janela é cliente do próprio servidor integrado e o craft do anfitrião passa
por `lan_crafting_click`, que já chamava o gancho (12 chamadas, uma por clique, vistas com um log temporário da glue). O
defeito real desse caminho era o do `01e5147ef5`, que tem teste de unidade puro no protocolo (9/9 em `item::hash`). A
prova J lê o inventário do lado do servidor (`data get entity`): barra feita na mesa pelo anfitrião sai com a tag,
renomeada continua viva; a cópia da janela fica na evidência.


## RBMK T04 — escalonador, fluxo cardinal/ReaSim e calor (18/11/2026)

Branch `rbmk-t04`, a partir da `main` @ `fa3c1df3` com a `rbmk-t03` (`7286787a`, que contém a `rbmk-t02`) fundida em
`6faa68cc` (decisão 1A do coordenador; pin do motor da `main`, `2187c3192b`, mantido). Sem mudança de motor. Decisões do
coordenador para este ticket: 1A (base), 2B (colisão `+0,25` da tampa fica fora: pendência de motor), 3A (dials por
dimensão fiéis ao `RBMKDials`), 4A (`SatelliteScan` num registro persistido por dimensão, TTL 300).

### O que mudou

| Item | Comportamento | Fonte Java |
| --- | --- | --- |
| Colunas no passo | As 18 colunas entram no passo (`persist::kind_of`): cada TE é nó dos fluxos (`OTHER` deixa passar, controle em nível 0 encerra) e vizinho do `moveHeat`. O que o save ainda não guarda passa como o TE recém-colocado: controle em nível e alvo 0 (T07 move), tanques secos (T08), outgasser que não processa (T09: os slots dele não são lidos) | `RBMKNeutronHandler.getNodes:213-238`/`runStreamInteraction:242-397`, `TileEntityRBMKBase.moveHeat:159-215` |
| Oclusão | `getHits` conta `isOpaqueCube` do bloco em cada altura da célula sem coluna da camada: a família RBMK pelo próprio override (`persist::java_opaque`: colunas `true`, canais `false`, console/guindaste/autoloader/painéis/pribris `false`, loader/inlet/outlet `true`); o dummy de uma coluna como o bloco da coluna (no Java o dummy é o próprio bloco), achado descendo até o núcleo; o resto pelo `solid_render` do motor (`ServerHost::block_opaque`, na glue e no `Deferred` do passo). Desconhecido continua congelando (D4) | `RBMKNeutronHandler.getHits:400-414`, `RBMKBase:58`, `RBMKRod:33`, `BlockDummyable:358`, `RBMKMiniPanelBase:42`, `RBMKDebris:21` |
| Dials por dimensão (3A) | As regras são as do save (no 1.7.10 a dimensão lê as do overworld, `DerivedWorldInfo`); o cache é por `World`: o início do servidor faz o do overworld (`createDials`, que esvazia os outros); a primeira leitura numa dimensão sem cache reescreve **a regra lida** com o padrão e monta o cache dela; `/ntmrbmk dial` refaz só o cache da dimensão de quem digitou; uma coluna colocada lê a altura da sua dimensão. A primeira leitura de um passo é a da primeira coluna da ordem lógica: canal com barra `dialDisableXenon` (`burn:157`), colunas com portas `dialColumnHeight` (`getPorts*:616`), as outras `dialReasimBoilers` (`moveHeat:161`). A altura que o motor usa ao colocar segue a dimensão do jogador | `RBMKDials.createDials:55-71`, `refresh:77-115`, `getGameRule:124-155`, `ModEventHandler.commandEvent:1088`, `MainRegistry.serverStart:665` |
| Radiação | `rad` do passo (barras destampadas, fluxos sobre e para dentro de colunas abertas, células de ar) vai ao campo por chunk com o commit (Onda 1). D2: incremento NaN não entra (o Java guarda e espalha o NaN; o JSON do campo gravaria `null` e perderia a chave inteira) | `TileEntityRBMKRod:153`, `RBMKNeutronHandler:305,417,421` |
| Satélite (4A) | `Effect::SatelliteScan` vira `RayEvent` (posição, `NEUTRON_EMISSION`, `expiresOn = tick + 300`) no registro da dimensão, com a semântica de `LinkedHashMap.put` (o mesmo lugar substitui onde está); `updateSystem` tira os vencidos (`tick > expiresOn`) quando `tick % 20 == 10`, só nas dimensões abertas. Guardado na chave `rbmk_rays`, gravada com os reatores. Desvio aprovado: o Java guarda só em memória | `SatelliteRayScan.reportEvent:89-91`, `updateSystem:94-99`, `ModEventHandler.worldTick:661`, `TileEntityRBMKRod:124` |
| Dump de passo | `rbmk::dump::ReactorDump`: a entrada exata de um passo (dials, RNG, fila, oclusão, colunas, pelos bits); `ntmtest rbmkdump` (só com `NTM_TEST_HOOKS=1`) grava a do próximo passo aceito. Dois dumps de um reator sem mão entre eles reproduzem um ao outro pelo núcleo puro | — |

### Desvios e limites registrados

- 2B: colisão `+0,25` da tampa (`RBMKBase.getCollisionBoundingBoxFromPool:128-148`) fora do T04: o
  `BlockCollisionRegistry` do motor não tem formas dinâmicas por BE. Pendência de motor pós-T02.
- `solid_render` como `isOpaqueCube`: blocos cujo `isOpaqueCube` do 1.7.10 diverge do moderno (folhas num servidor
  dedicado do 1.7.10 são opacas; no integrado seguem os gráficos "fancy"; aqui nunca), e blocos de mod sem paridade
  de forma, seguem o fato do motor.
- Um motor, uma altura de colocação: jogadores em duas dimensões cujos caches diferem colocam colunas na altura da
  dimensão do primeiro jogador. Uma coluna posta por `/setblock` também conta como leitura da altura (no Java só a
  colocação do jogador lê).
- Ordem de leitura por dimensão: a primeira leitura é a do primeiro passo de reator da dimensão na ordem dos grupos
  (camada, componente, ordem de registro), não a da lista de TEs carregados do Java (D1).
- O que o save não guarda das dez colunas novas no passo volta ao valor do TE novo a cada tick (temporizador do
  cooler, `ventDelay` do boiler): sem fluido não há efeito observável; T07/T08/T09 persistem o que for deles.
- **Um tick de espera ao abrir o mundo: corrigido na revisão** (ver "Correções da revisão" abaixo). Medido na entrega:
  o save guardava relógio e estado juntos (271 passos em 271 ticks), mas depois de abrir o reator dava um passo a menos
  que o relógio (25 em 26), porque a máquina só era adotada no primeiro tick do BE, um tick depois da carga.

### Validação (19/11/2026)

Código: `ac410fab` (passo, oclusão, dials, satélite, D2, dump), `47cc7ca8` (testes), `6e086f21` (oráculo),
`f21bfa3a` (predicado único de expiração), roteiros de prova e docs nos commits seguintes. Binário release
`horizon-client` SHA-256 `aaa9076a25b2ff4a9079505faad1f1fea3e1525216163b93c5872fddcc46a814` (motor `2187c3192b`, o pin
da `main`; fontes do bundle iguais a `ac410fab`; o que veio depois só muda testes, roteiros, docs e a forma do
predicado de expiração, mesmo comportamento), pacote `ntm.hmod` `ea71442ae806f6082f29d109d2388bfb23b2f32e09d4b1436c3aeaa85c1671d6`.
Evidências em `/mnt/ssd120/sacer/so/rbmk-t04/evidence/` (JSON das aberturas, dumps, replays, mutações, logs).

| Prova | Resultado |
| --- | --- |
| U: oráculo Java | `run_flux.py --check` igual byte a byte (`593ceb71…`): 43 cenários, os 39 anteriores com resultados idênticos e 4 novos (eficiências 0; canais vazios no caminho, todos os tipos `OTHER` e um segundo reator fora de alcance; fim de stream com controle na última célula e obstrução total/parcial/nenhuma na célula de irradiação; todos os dials físicos fora do padrão com controle se movendo). `rbmk_flux_parity` compara tick a tick, por bits. Suíte Python do oráculo: 21/21 |
| U: unidade | Lib `ntm-core`: 759 (debug, suíte inteira) + integração 118 = 877, 6 ignorados, 0 falhas; testes RBMK 51 depois dos últimos ajustes. Novos: oclusão (`isOpaqueCube`: vidro, console, painel, dummies de canal não contam; pedra 3/4 do fluxo, dummies de blank metade, bits exatos), as 18 colunas como nós (storage e caldeira deixam passar bit a bit, controle em nível 0 encerra) e no calor, primeira leitura de dial por tipo de coluna, cache por dimensão (comando, colocação no End, reabertura com leitura do primeiro canal no Nether), satélite (relato a cada 200, substituição no lugar, expiração estrita em `% 20 == 10`, save e leitura), NaN sem efeito no campo, camada corrompida no save, dump e replay |
| U: mutações | 15/15 pegas (`evidence/mutations-1.log`, `mutations-2.log`): `kind_of` só com as oito do T01; canal opaco; dummy sempre opaco; tabela RBMK ignorada; host do passo sem `block_opaque`; primeira leitura sem gravar o padrão; comando refazendo o overworld; `createDials` sem esvaziar os caches; primeira leitura sempre `moveHeat`; relato de satélite somado em vez de substituído; expiração em `>=` (sobreviveu na 1ª passada: o predicado estava duplicado; unificado em `f21bfa3a` e pega); expiração todo tick; chave `rbmk_rays` não gravada; NaN chegando ao campo; dump sem a fila |
| Clippy / rustfmt | `ntm-core` todos os alvos `-D warnings`: 0; glue (`horizon-client`, `horizon-client-lib`, release, no stage do build): 0. Arquivos novos (`dump.rs`, `rbmk_t04_tests.rs`) `rustfmt` limpos; os existentes seguem com as diferenças de antes (`docs/estado-da-integracao.md`) |
| `save_model` (release) | 11/11 |
| `keep_world_cost` (release) | 3 execuções, 0,17–0,30 ms por tick com 0, 5 mil e 50 mil gerações (a chave `rbmk_rays` vai com a `rbmk`); uma primeira execução com o jogo e uma compilação nos mesmos 2 núcleos passou de 1 ms e foi refeita |
| J: reator 15×15 (rodada 3, `run-rbmk-t04-proof.py`) | **play 23/24, reopen 5/5**. 225 colunas colocadas e 59 canais abastecidos pela mão, um só reator com 12 tipos no passo (refletor, absorvedor, blank, caldeira, storage, resfriador, aquecedor, moderador, os dois canais e os dois controles). Com `dialReactivityMod 0,35` (escolhido passando o núcleo sobre o dump do próprio layout: no padrão 1,0, sem caldeiras, ele dispara e toda coluna passa de 1500 em 2 min, o que a rodada 1 mostrou) opera: fluxo nos 59 canais, 276 → 303 °C em 25 s. `dialFluxRange 3` chega à entrada do passo seguinte e volta a 5. Eventos de satélite: todos `NEUTRON_EMISSION`, `expiresOn ≡ 100 (mod 200)`, mantidos na reabertura (85) |
| J: replay contra o núcleo | Cadeias da rodada 3: `a` 515 passos em 515 ticks, `b-1→b-2` 4249/4249 (com o jogador longe, nas provas de oclusão e radiação), `b-3→b-4` 312/312, todas bit a bit (225 colunas, RNG, fila); `b-2→b-3` atravessa a reabertura: 34 passos em 35 ticks (o tick de espera acima). Rodadas 1 e 2 iguais (516, 4039+1, 212; 512, 4224, 34+1, 313; terceira abertura `c` 111/111). Do dump antes do fim ao estado em disco: 271/271 |
| J: oclusão sobre os blocos do motor | 12/12 nas três rodadas: aberto, vidro, folhas, laje, escada, painel de vidro, grades e os dummies de um canal → nenhuma; pedra (altura 1), terra (0), os dummies de um blank (0 e 1), quatro pedras (0–3) |
| J: radiação | Prova curta (pares em chunks vizinhos lidos num comando, `evidence/j-rad-only.log`): o chunk do par destampado acima do tampado nas 5 leituras (1280/1140, 1426/1271, 1185/1054, 1329/1184, 1494/1333), ≈ +12%; o núcleo, sobre os dumps dos pares da rodada 3, dá +11% de `incrementRad` em 400 passos (8168 contra 7354). Na rodada 3 a comparação com leituras separadas por 6 ticks saiu invertida (954/987): a leitura do campo, não a física |
| J: 3A em jogo | Uma coluna posta no Nether (sem cache) reescreveu `dialColumnHeight` para 4 enquanto o overworld, com cache em 6, colocou a coluna seguinte do jogador com 6 blocos |
| C (`run-rbmk-t03-proof.py`, cenário do T03, no binário do T04, `HORIZON_NO_EAGER_SAVE=1`) | 4/4 cortes, NTM igual ao controle vanilla item a item nas duas reaberturas: `kill`, `kv_committed` e `players_saved` 3 = 3; `live_chunk:2,0` 2 = 2 (o mesmo resíduo vanilla do T03); sem duplicação, armazenamento sem item em todas as aberturas (a chave nova `rbmk_rays` não guarda item). Relatório em `evidence/c-report.json`; mundos apagados |
| C: saldo térmico e de fluxo | O replay das cadeias reproduz bit a bit, a cada tick, o calor de cada coluna (produção da barra, `moveHeat`, resfriamento passivo, limites) e os buffers de fluxo (`fluxQuantity`/`lastFluxQuantity` e a fila de 240 fluxos) do núcleo validado pelo oráculo; nenhum item muda pelo passo (as barras só mudam pela própria tag) |

### Correções da revisão (19/11/2026)

A revisão independente aprovou a entrega (0 reprovações, sem P1/P2). Decisão do usuário: corrigir o tick de espera com a
opção (a) e os P3 apontados, sem sync com o upstream (motor `2187c3192b`).

| Item | Como fica | Commit |
| --- | --- | --- |
| Tick de espera ao abrir (opção a) | A glue lê, na carga de cada chunk (o gancho `chunk_load` do motor: "seus blocos e BEs já estão lá"), as máquinas do chunk com o estado salvo e as entrega ao servidor (`Server::native_chunk_machines_loaded`), que as adota no começo do tick seguinte, depois das obrigações do chunk e antes do passo. A adoção no primeiro tick do BE continua para o que a carga não trouxe (um BE salvo antes do primeiro tick) | `274cac6b` |
| P3(a): radiação na prova principal | Os pares aberto e tampado em chunks vizinhos, lidos num único comando, cinco vezes | `99b1947c` |
| P3(d): corte estrito sobre o estado novo | Cenário C do T03 com uma regra de dial reescrita antes do ato (o controle reescreve uma gamerule vanilla) e os eventos de satélite em jogo, cortado entre `kv_committed` e `chunks_written` | `e32ed906` |
| P3(e): dummy órfão | Teste: uma sequência de dummies sobre nenhuma coluna, ou mais longa que qualquer coluna, conta como o bloco opaco do `RBMKBase` | `b4ce9677` |

Testes: `machines_adopted_as_their_chunk_loads_lose_no_step_at_an_opening` abre o mundo como o motor (as máquinas só pela
carga) e segue igual ao mundo que nunca fechou, 23 ticks e 23 passos; o mesmo mundo com a adoção atrasada um tick (o
caminho de antes) fica um passo atrás. `an_orphan_dummy_counts_as_opaque`: dois dummies órfãos tomam metade do fluxo.
Mutações: 3/3 pegas (adoção na carga sem efeito; órfão sobre um bloco transparente; sequência além do alcance
transparente). Binário release `ebe6658680d151cd28555ea37f2cc665d3822dd3a9b77ef6421480d0a7dcd5ca` (motor `2187c3192b`,
fontes do bundle de `274cac6b`; o que veio depois só muda testes, roteiros e docs), pacote `ntm.hmod` `ea71442a…`
(conteúdo inalterado).

| Prova | Resultado |
| --- | --- |
| U | Suíte `ntm-core` em debug 879 ok, 6 ignorados, 0 falhas; testes RBMK 53; `save_model` release 11/11; `keep_world_cost` 0,16–0,21 ms por tick; Clippy `-D warnings` do `ntm-core` (todos os alvos) e da glue (release, stage): 0 |
| J (rodada 4, `evidence/j-run4/`) | **play 24/24, reopen 6/6**: inclui `open_columns_leak_more_every_read` e `satellite_phase_kept` (85 eventos relatados depois da abertura, todos em múltiplos de 200). Replay com tolerância zero de espera (`RBMK_T04_WAIT=0`): `a` 515/515, `b-1→b-2` 4401/4401, **`b-2→b-3` através da reabertura 67/67** (antes 34 em 35), `b-3→b-4` 312/312, **`b-4→b-5` através de uma terceira abertura 335/335**, `b-5→b-6` 115/115, todos bit a bit |
| C: cenário do T03 no binário novo (`HORIZON_NO_EAGER_SAVE=1`, `evidence/c-report-r2.json`) | 4/4 cortes, NTM igual ao controle vanilla item a item nas duas reaberturas: `kill`, `kv_committed`, `players_saved` 3 = 3; `live_chunk:2,0` 2 = 2 (o resíduo vanilla de sempre); sem duplicação, armazenamento sem item. A adoção na carga não muda o saldo |
| C: corte estrito sobre o estado novo (`run-rbmk-t04-c-proof.py`, `HORIZON_NO_EAGER_SAVE=1`, `evidence/c-strict-report.json`, `c-strict-rules.json`) | Cenário C do T03 com `dialFluxRange` 5 → 7 logo antes do ato (o controle: `random_tick_speed` 5 → 7) e os eventos de satélite em jogo, cortado nos 5 pontos entre `kv_committed` e `chunks_written`: **5/5**, NTM igual ao controle item a item (3 = 3 em quatro cortes; `chunks_written` 2 = 2, a mesma última escrita que o vanilla perde); armazenamento sem item. A regra lida depois das duas reaberturas é a mesma nos dois mundos em todos os cortes: 7 em `kv_committed`, 5 (o valor do último save) nos três `region_record` e em `chunks_written`; nunca outro valor. Os eventos de satélite voltam legíveis e em fase (múltiplos de 200) em todos os cortes. Uma primeira rodada reprovou o controle por erro do roteiro (o nome antigo `randomTickSpeed`, depois um valor de base não salvo); corrigido em `85803721` e `992b5b26` |

### P3 da re-revisão (19/11/2026)

A re-revisão aprovou as correções (sem P1/P2) e apontou três P3, fechados um por commit:

| Item | Como fica | Commit |
| --- | --- | --- |
| (c) contagem de adotadas na carga | `native_chunk_machines_loaded` conta só a máquina que a carga deixou viva no servidor: um fantasma (lote de lápide já aplicado) e uma geração que um lote pendente ainda nomeia não entram na conta do log | `d938dd14` |
| (b) teste de servidor da adoção na carga | `a_chunk_load_leaves_a_ghost_unadopted_and_uncounted` (lápide aplicada: conta 0, um só fantasma, o bloco sai no tick, nada retido) e `a_chunk_load_leaves_an_older_generation_a_pending_batch_names` (lote pendente nomeando a geração antiga: conta 0, geração mantida, a geração atual adotada conta 1). Mutação "fantasma contado como adotado": pega | `ef469382` |
| (a) eventos de satélite no corte estrito | Cada leitura grava o tick, as posições dos eventos e os canais com barra naquele momento, e afirma o conjunto que a leitura pode ter: evento só onde um canal tem barra (`TileEntityRBMKRod:122-124`), em fase (`expiresOn ≡ 100 mod 200`), nenhum vencido há mais de 20 ticks. Nenhum evento é leitura válida quando nenhum canal pegou fluxo no último múltiplo de 200 | `5cab42ec` |

Corte estrito refeito com o roteiro novo (`evidence/c-strict-p3a/`), NTM igual ao controle vanilla item a item:
`kv_committed` 3 = 3, eventos lidos = os três canais com barra (tick 1431, todos com `expiresOn` 1700), e 0 na segunda
reabertura (tick 2139: o yield de P não muda em 3 s, o reator sem fluxo naquele momento); `chunks_written` 2 = 2 (a
mesma última escrita que o vanilla perde), 0 eventos na primeira reabertura (tick 948: o corte perdeu a barra de P, S
sem par, S e Q sem fluxo) e um evento em Q (`expiresOn` 1700) na segunda. Regra do dial: 7 e 7 em `kv_committed`, 5 e
5 em `chunks_written`, os mesmos valores do controle.

## RBMK T05 — acesso ao combustível e política contextual de slots (19/11/2026)

Branch `rbmk-t05` (a partir de `rbmk-t04` @ `845d0681`) e, no motor, `rbmk-t05-engine` (a partir do pin da `main`,
`2187c3192b`; sem sync com o upstream). O binário das provas sai de `tools/build-native.sh` com
`HORIZON_NEXT=<worktree do rbmk-t05-engine>` e `NEXT_REV=6e13d4271c`; o pin de `native/ntm/compatibility.json` continua o
da `main` até o coordenador fundir a branch do motor.

### O que mudou

| Caminho | Comportamento | Fonte Java |
| --- | --- | --- |
| Menu do canal em sobrevivência | Todo gesto sobre o slot do canal (clique esquerdo/direito, shift-click, tecla numérica, Q, drag, coleta) é recusado enquanto o casco da barra passa de 200 °C (`<= 200` é frio; NaN não é frio): nada muda no BE, na mão nem no cursor. Canal vazio, ou com outro item, é frio (o slot é um `Slot` vanilla e aceita qualquer item). Regra em `persist::cold_enough_for_manual`, chamada por `native::slot_allows` só para os quatro canais e a origem `Manual` | `ContainerRBMKRod.slotClick:32-48`, `transferStackInSlot:52-76` (checagem em `:61`), `TileEntityRBMKRod.coldEnoughForManual:100-105` |
| Criativo | A regra não vale: `ModSlotFilter::creative_bypass` (extensão do motor, `6e13d4271c`) pula a `policy` nos gestos de menu de um jogador em criativo, no anfitrião e no hóspede LAN; filtros estáticos continuam e a automação nunca pula | `!player.capabilities.isCreativeMode` nas duas checagens |
| Mão sobre o bloco | Sem mudança (T01): a barra na mão entra num canal vazio, quente ou fria, uma da pilha com a tag inteira, debitada só em sobrevivência, com `upgradePlug` só no ramo aceito; canal cheio abre o menu, sem som | `RBMKRod.onBlockActivated:82-86` |
| Automação | Sem mudança (T02): canais e aquecedor não mostram slot a funis/proxies | `TileEntityRBMKSlottedBase.getAccessibleSlotsFromSide` |
| Aquecedor | Política própria (antes compartilhava a dos canais): não ganha a regra de casco | — |
| Limiares prontos | `cold_enough_for_autoloader` (`<= 1000`, T12) e `hot_enough_to_melt_on_break` (`>= 1500`, T16), testados nas fronteiras | `coldEnoughForAutoloader:94-99`, `invalidate:108-114` |

### Desvio registrado (decisão A do coordenador, 19/11/2026)

- **Coleta por clique duplo**: no Java o clique duplo num slot do jogador (`slotClick` modo 6, índice ≠ 0) junta pilhas
  iguais de todos os slots, inclusive o do canal quente, sem passar pela checagem de calor. Aqui a coleta consulta a
  política (`Manual`) e não tira a barra quente. Reproduzir exigiria a política saber o gesto, uma extensão a mais do
  motor. A diferença é inobservável: barras não empilham (`maxStackSize` 1) e o modo 6 só junta até o máximo do
  cursor, então o cursor que dispara a coleta já está cheio. Decisão A: manter assim, sem estender o motor.

### Validação

Commits: `0ea70f10` (regra e limiares), `fd123d89` (testes), `2a25bf92` (glue), `6511d31d` (docs da política),
motor `6e13d4271c`. Binário release `a387888669a5402c94d872647e3a6e2fde33aafb268e5b13a5f9b8ccc8e19b80`, pacote
`ntm.hmod` `ea71442a…` (conteúdo inalterado). Evidências em `/mnt/ssd120/sacer/so/rbmk-t05/evidence/`.

| Prova | Resultado |
| --- | --- |
| U: fronteiras | `the_hull_thresholds_are_javas_comparisons`: 200 e 1 ulp acima, 1000 e 1 ulp acima, 1 ulp abaixo de 1500 e 1500, NaN, ±infinito para as três regras; `an_empty_slot_or_another_item_is_cold`; `a_rod_without_the_key_reads_its_defaults_or_zero` (sem tag: padrões, casco 20; tag sem a chave: 0) |
| U: ações e origens | `a_channel_menu_follows_its_rods_hull` (4 canais × 5 cascos × retirar/inserir pedra/inserir barra × `Manual`/`Automation`), `the_other_columns_menus_ignore_a_hot_rod` (aquecedor, storage, caldeira, blank), `a_hot_rod_goes_in_by_hand_and_only_the_accepted_branch_sounds` (recusa silenciosa, uma da pilha com a tag, canal cheio segue ao menu); motor: `a_creative_bypass_skips_the_policy_for_a_creative_players_gestures_only` |
| U: mutações | 11/11 pegas (`mutations-t05.log`): `< 200`; NaN frio; canal vazio quente; `< 1000`; `> 1500`; casco de outro item; regra em toda coluna; regra na automação; sem regra; bypass em todo filtro; bypass na automação |
| U: suíte | Lib `ntm-core` 769 ok, 6 ignorados, 0 falhas + integração toda ok; `save_model` release 11/11; `keep_world_cost` 0,24–0,25 ms; Clippy `-D warnings` do `ntm-core` (todos os alvos) e da glue: 0; motor: `mod_slot_filters` 3/3 e testes de menu do `horizon-client` 36 ok, 4 ignorados, 0 falhas |
| J: anfitrião e hóspede LAN (`run-rbmk-t05-proof.py`, `evidence/j-run3/`) | **56/56**, dois clientes (anfitrião com o mundo aberto em LAN e um hóspede que entrou por `connect`), cada um em sobrevivência com barra quente (casco 1200) e fria (150): **quente** — clique, clique direito, shift-click, tecla numérica e Q recusados, nada muda no BE, na mão, no cursor nem no chão; **fria** — clique tira a barra inteira (tag igual) e devolve, shift-click sai e volta, tecla numérica troca com a pedra da hotbar e destroca, shift-click com o inventário cheio não move nada, Q joga uma; **vazio** — drag de duas pedras deixa uma no canal; **criativo** — a barra quente sai por clique e por shift-click e volta; **mão** — barra quente e fria entram em canal vazio, uma da pilha, com um `upgradePlug` por inserção (anfitrião ouviu 5, as 5 inserções perto dele; hóspede 2, as suas); canal cheio abre o menu, sem som; **funis** — o de cima não põe a barra no canal vazio, o de baixo não tira do canal frio. O casco fica onde a prova pôs (`dialHeatProvision` 0, core = casco). Rodadas 1 e 2 caíram por motivos da prova, não do mod: cliente sem responder ao `status` por mais de 6 min com carga média 84, e um BE vazio sem `Items`; corrigido no executor e no roteiro |

### P3 da revisão do T05 (20/11/2026)

A revisão independente aprovou os P3 do T04 e o T05 (sem P1/P2) e apontou dois P3, um commit cada:

| Item | Como fica | Commit |
| --- | --- | --- |
| (a) mínimo nos eventos de satélite do corte estrito | O gancho de teste guarda em memória, por dimensão, os canais com barra que tomaram fluxo no último passo num múltiplo de 200 (o `lastFluxQuantity` depois do passo, à parte dos relatos), e `ntmtest rays` o mostra (`flux200=`). Quando a leitura dele é a do último múltiplo antes da leitura, cada canal dele precisa ter o seu evento, relatado naquele tick (`expiresOn` = tick + 300). Antes de um passo assim depois de uma abertura nada é exigido | `723f22d3` |
| (b) frase do desvio do clique duplo | "Inobservável: barras não empilham (`maxStackSize` 1) e o modo 6 só junta até o máximo do cursor" | `1dd73bc1` |

Corte estrito refeito com o mínimo (binário `4ccb24bc…`, `evidence/c-strict-p3a-min/` em `rbmk-t05`): `kv_committed`
3 = 3, e nas duas reaberturas a leitura do servidor (ticks 1200 e 1600) nomeou os dois canais que tomaram fluxo, ambos
com o evento daquele tick; `chunks_written` 2 = 2, sem canal com fluxo nos múltiplos lidos (1000 e 1200), os eventos
restantes de antes da abertura. Mesmo resultado do controle vanilla item a item.

## RBMK T06 — base de telas, sessões e codec de comandos (20/11/2026)

Branch `rbmk-t05` (continua sobre o T05 aprovado), motor `rbmk-t05-engine` @ `6e13d4271c` **sem mudança nova**: o T06
usa só o que o motor já oferece (`net_from_client`/`native_net_send_to_server` e `native_net_send_to_player`/
`net_from_server`, o canal de mensagens de mod, até 30 000 bytes; `menu_extra_data`/`native_screen_extra_data`;
`player_logged_out`). O jogador do anfitrião é hóspede do próprio anfitrião, então o mesmo caminho serve aos dois.

### O que mudou

| Parte | Comportamento | Fonte Java |
| --- | --- | --- |
| Campo de texto | `screens::field::TextField`: o `GuiTextField` do 1.7.10 como o Java edita (texto em unidades UTF-16, cursor e outra ponta da seleção, a folga do `writeText` — com a seleção feita de trás para a frente num campo cheio o caractere digitado some, como no Java —, palavras, teclas 14/199/203/205/207/211, Ctrl+A/C/V/X com a área de transferência do jogo, `ChatAllowedCharacters`, o cursor posto pelo clique) | `GuiTextField`, `ChatAllowedCharacters` |
| Regra de salvar | `screens::number`: `NumberUtils.isNumber` do commons-lang3 3.3.2 (inclusive hexadecimal, octal com zero à frente e os sufixos `d`/`f`/`l`), `Double.parseDouble`, `MathHelper.clamp_double` e o `(int)` | `GUIRBMKControlAuto.mouseClicked:93-138` (a regra em `:104-118`) |
| Codec | `screens::control`: comando (sessão, geração do BE, posição, número de sequência, campos), snapshot (o pacote de rede do TE) e concessão (a sessão e o primeiro snapshot, nos dados extras do menu), binários e versionados; tags NBT com os getters do `NBTTagCompound` (número de outro tipo convertido, `double` lido como `int` arredondado para baixo, chave ausente = 0); leitura estrita (versão, tipo, tag, UTF-8, chave repetida, bytes sobrando, limites) | `NBTControlPacket`, `NBTTagCompound`, `NBTTagDouble/Int/Byte` |
| Sessão | Uma por jogador: abre com a tela, termina com a próxima tela ou com a saída do jogador. Comando aceito só da sessão atual, para o lugar e a geração do BE em que ela abriu, com número de sequência acima do último e dentro do alcance do `hasPermission` (distância menor que 20 do canto do bloco, mesma dimensão); qualquer outro é recusado inteiro, sem mudar bloco nem jogador | `IControlReceiver.hasPermission` (`TileEntityRBMKControlAuto:35`) |
| Aplicação | O comando aceito espera, guardado pela geração do BE, a próxima transação da máquina, e entra antes do passo (como um botão de menu). No Java entra quando o pacote chega, entre dois ticks | `NBTControlPacket.Handler.onMessage` |
| Piloto: controle automático | Tela sem slot de máquina (`ContainerRBMKControlAuto`): os quatro campos (3, 3, 4, 4 caracteres; os valores guardados como `int` ao abrir), salvar e as três curvas. A coluna guarda as quatro faixas e a curva (`AutoControl`, pelos bits; save anterior lê os padrões do TE) e as recebe como o `receiveControl`. Os botões ±1/±10 (o atalho de antes) saíram. Nível, alvo e energia ficam em 0 até o T07 mover a barra | `GUIRBMKControlAuto`, `TileEntityRBMKControlAuto.receiveControl:131-146`, `serialize:111-118` |

### Desvios (decisões confirmadas pelo coordenador, 20/11/2026: as quatro abaixo ficam como estão)

- **Texto que passa no `isNumber` e quebra o `parseDouble`** (`1L`, `0x1`, 312 das 111 151 cadeias do oráculo): no
  1.7.10 o clique em salvar derruba o cliente. Aqui o salvar para nesse campo e não manda nada; os campos antes dele já
  aparecem como salvos (o que o Java fez antes de cair).
- **Índice de curva fora de 0..2** (`abs(i) % 5` dá 3 ou 4; `abs(Integer.MIN_VALUE)` é negativo): no Java,
  `ArrayIndexOutOfBounds` no tratador do pacote. Aqui o comando é recusado inteiro (D2). Nosso cliente nunca manda isso.
- **Sessão**: o Java aceita um `NBTControlPacket` de qualquer jogador a menos de 20 blocos, com ou sem a tela aberta.
  Aqui o comando precisa da sessão atual (critério de aceite do plano: "rejeitar comando de sessão antiga"). O cliente
  só manda com a tela aberta, então a diferença só aparece para um cliente modificado.
- **Momento**: até o começo do tick seguinte, em vez de entre ticks.
- **Número de sequência gasto antes do D2**: `Sessions::accept` toma o número do comando antes de o servidor testar se o
  `receiveControl` o recusaria (índice de curva ou grupo inválido, D2). Um comando recusado assim gasta o seu número;
  é inofensivo, porque o cliente sempre manda o próximo com um número maior.
- **Comando no último tick**: um comando aceito no último tick antes de o servidor parar espera uma transação que não
  vem e se perde (a fila fica só em memória); no Java ele já teria sido aplicado ao chegar e iria no save. É o custo da
  decisão "aplicar no começo do tick seguinte"; a janela é de um tick.
- **D6 (spike)**: os snapshots tipados por mensagem de mod estão confirmados por este ticket; o estado das pelotas por
  `custom_data` tem caminho no motor para o desenho (`item_model_property`, modelos condicionais avaliados pelo mod, e
  `custom_model_data`), para o T14.

### Validação

Commits: `fe52ebc2` (campo e regra de salvar), `3d3703b4` (codec e sessões), `14c78949` (ajustes do controle
automático), `0ce5925b` (servidor), `5785bf07` (testes), `7c510699` (glue), `2a8cce09` (conteúdo), `55c5f567` (linhas
citadas), `77cc9129` (correção: tela sem slot de máquina sem menu), `49e9455c` (roteiro J), `4495c11f` (correção: a
sessão não abria, `native_server_players` não lista o jogador enquanto o menu dele abre), `63235245` (gancho de teste
para o hóspede). Evidências em `/mnt/ssd120/sacer/so/rbmk-t06/evidence/`.

| Prova | Resultado |
| --- | --- |
| U: oráculo Java | `NumberOracle.java` (commons-lang3 3.3.2 do 1.7.10, JDK 21) sobre as 111 151 cadeias de até 4 caracteres de `018.-+eExXdDfFlLa `: `isNumber`, os bits do `parseDouble` e o salvar com teto 100 e 9999 iguais em todas; 312 passam no `isNumber` e quebram o `parseDouble` |
| U: unidade | Campo (cota de `writeText` com o defeito da seleção invertida, palavras, área de transferência, filtro, clique, unidades UTF-16), codec (ida e volta pelos bits de NaN/−0/±∞/extremos, toda mensagem cortada recusada, bytes sobrando, versão, tag, chave repetida, UTF-8, limites), sessões (sessão atual, lugar, geração, repetição, alcance 20 e dimensão), getters do `NBTTagCompound`, `receiveControl` do automático (índices 3/4/MIN recusados), servidor (aplicado no tick seguinte, recusa sem mudar nada, comando de coluna trocada descartado, snapshot só quando muda, save anterior) |
| U: suíte (árvore limpa em `6eb5941c`) | `ntm-core` debug 913 ok, 6 ignorados, 0 falhas; `save_model` release 11/11; `keep_world_cost` 0,16–0,19 ms; Clippy `-D warnings` do `ntm-core`: 0. Clippy da glue: 0, rodado sobre o stage já com o T07 (T06 + T07 juntos) |
| U: mutações | 16/16 pegas (`evidence/mutations-t06.log` em `rbmk-t05`) |
| J: anfitrião e hóspede LAN (`run-rbmk-t05-proof.py --script native_rbmk_t06.py`, `evidence/j-run5/`) | **24/24**: cada jogador digita e salva os quatro campos (75, 20, 1500, 300 guardados), a regra do Java (`12345` vira 100 no limite de 3 caracteres e teto, `abc` vira 0, `9.9` vira 9), `1L` não manda nada, a curva escolhida, repetição recusada (`Repeated`), campos reabertos do servidor (salvar sem digitar mantém), sessão velha recusada (`OldSession`) sem mudança, coluna trocada nova e o comando da antiga recusado (`Generation`). Binário `0ef9ea6f…` (com as correções e o T07; os caminhos do T06 são os mesmos), pacote `0f9c0154…`. As rodadas 1–4 caíram pelo roteiro (`lan open` sem porta) e pelos dois defeitos corrigidos acima (a tela sem menu; a sessão que não abria), e por um gancho de teste cuja resposta não volta ao hóspede |

### P3 da revisão do T06 (20/11/2026)

A revisão independente aprovou o T06 (sem P1/P2) e apontou quatro P3, um commit cada:

| Item | Como fica | Commit |
| --- | --- | --- |
| (a) teto da fila | A fila guarda, por coluna, um de cada tipo: o último alvo, ajustes ou curva (valores postos de vez), e os toques de grupo da barra manual compostos num mapa do grupo antes para o grupo depois (o grupo é um alternador: guardar só o último toque erraria, porque dois toques iguais seguidos se anulam no Java). Teste exaustivo: toda sequência de até quatro comandos, de cada grupo inicial, igual aos comandos um a um; mil comandos num tick deixam no máximo duas entradas. Mutantes 3/3 (`evidence/mutations-t06-p3a.log`) | `f1073087` |
| (b) número gasto antes do D2 | Registrado em "Desvios" | `c7e07dfc` |
| (c) comando no último tick | Registrado em "Desvios" | `c190450d` |
| (d) prova J | Recusa por distância: o comando da sessão atual, reenviado com o número seguinte (`ntmtest screen next`) por um jogador a 25 blocos, é recusado (`TooFar`) sem mudar nada, no anfitrião e no hóspede. Persistência: uma coluna ajustada pelo anfitrião volta com os ajustes depois que o mundo é fechado e aberto de novo (`--reopen` do executor). `evidence/j-run6/`: **29/29 e 1/1 na reabertura** (binário `bin-d`) | `29a8c13c`, `f6d77fc0` |

## RBMK T07 — barras de controle manual, automática e energizada (20/11/2026)

Branch `rbmk-t05` (continua sobre o T06), motor `rbmk-t05-engine` @ `6e13d4271c` sem mudança nova. O núcleo do passo já
movia as barras (validado contra o oráculo no T1); o T07 liga o estado, as telas e a energia.

### O que mudou

| Parte | Comportamento | Fonte Java |
| --- | --- | --- |
| Estado | `ControlState` na coluna das cinco barras: `level` e `targetLevel` (e o `startingLevel` e o grupo da manual), pelos bits, como o `writeToNBT` grava; a energia das ReaSim só em memória, porque o Java não grava `power` (uma barra reaberta começa vazia). Save anterior lê como o TE novo | `TileEntityRBMKControl.readFromNBT/writeToNBT:128-142`, `TileEntityRBMKControlManual:92-115` |
| Passo | Cada barra entra no passo como o seu TE (nível, alvo, partida, energia, as faixas e a curva da automática) e volta com o commit: `speed * dialControlSpeed` por tick até o alvo, o surto da manual ao descer, a curva da automática sobre o calor da coluna, nível 0 encerrando o fluxo | `TileEntityRBMKControl.updateEntity:76-109`, `TileEntityRBMKControlManual.getMult:52-62`, `TileEntityRBMKControlAuto.updateEntity:40-79` |
| Energia | As ReaSim (`rbmk_control_reasim`, `rbmk_control_reasim_auto`) recebem da rede pela face de baixo (`Ports::Down`), até 50 000; andam só com 5000 ou mais e pagam 5000 por tick em que andaram | `isPowered`, `canConnect` (DOWN), `consumption`, `maxPower` |
| Tela da manual | Cinco níveis (1 − 0,25k) e cinco grupos, cada um um comando da sessão (T06): `level` vira o alvo (a partida onde a barra está), `color` alterna o grupo (`abs % 5`; o mesmo de novo tira). Barra de nível desenhada com `(int)(56 * (1 - level))`, grupo, luz de energia e dicas (nível em %, energia com `getShortNumber`) do snapshot | `GUIRBMKControl`, `TileEntityRBMKControlManual.receiveControl:70-90` |
| Telas | As cinco mostram nível, alvo e energia reais; a automática ganhou a barra de nível, a curva e a luz desenhadas do snapshot. O JSON das telas ficou só com a textura, o título e os rótulos | `GUIRBMKControlAuto.drawGuiContainerBackgroundLayer` |
| Copiar configurações | `rbmk::screen::settings`/`paste_settings`: o grupo da manual (sem grupo: composto vazio, que tira o grupo da outra) e as faixas e a curva da automática (`grabEnumSafely`) | `ICopiable.getSettings/pasteSettings` das duas |

### Desvios e decisões para o coordenador

- **Energia não gravada** (como o Java): uma barra ReaSim reaberta começa sem energia. A alternativa (gravar, uma
  correção de durabilidade) muda o comportamento visível de quem fecha o mundo com a barra andando.
- **Ferramenta de cópia** (`ItemSettingsTool`, `settings_tool`): não existe no port. As funções de copiar e colar das
  barras estão prontas e testadas. Decisão do coordenador (20/11/2026): ticket próprio (T07-F no plano), depois do T07
  aprovado.
- **Prioridade da rede**: as barras são `ConnectionPriority.LOW` no Java; a rede do port não tem prioridades (limite já
  registrado da rede). Quando a rede de energia em jogo ganhar prioridades, as duas ReaSim (`ControlState` como
  `EnergyNode`) precisam declarar LOW.
- **Energia fora do passo**: a rede entrega a energia às barras fora do passo do reator, então a checagem de 5000 do
  passo usa o buffer como o tick anterior o deixou. No Java a rede (`transferPower`) e o `updateEntity` da barra correm
  na ordem dos TEs do mesmo tick; com o buffer exatamente abaixo de 5000 a barra pode andar um tick depois do Java.
- **AZ-5**: é um comando do console (T10); o T07 cobre o surto da barra que desce para 0.
- **Comandos de rádio (RTTY)** e OpenComputers das barras: T13 e exclusão D7.

### Validação

Commits: `d06ac206` (estado das barras), `0cb60b12` (o passo move as barras), `e61fc737` (comandos da manual,
snapshots reais, copiar configurações), `64ab45a9` (testes), `e18aa0dd` (glue: a tela da manual e o estado vivo),
`093679aa` (conteúdo: as telas só com a textura), `29a8c13c` (ganchos `ntmtest power` e `ntmtest screen next`),
`4f4a1bb1` (roteiro J). Evidências em `/mnt/ssd120/sacer/so/rbmk-t07/evidence/`.

| Prova | Resultado |
| --- | --- |
| U: unidade | Manual na velocidade do Java até o alvo e parada nele; surto ao descer; ReaSim só anda com energia e paga 5000 por tick em que anda, parada não paga; automática segue a curva sobre o calor da coluna (faixas iguais e invertidas); barra em 0 corta o fluxo e fora deixa passar; coluna anterior ao T07 lê como barra nova; grupo alternado, copiar e colar como o Java; `getShortNumber` |
| U: suíte (`29a8c13c`) | `ntm-core` debug 924 ok, 6 ignorados, 0 falhas; `save_model` release 11/11; `keep_world_cost` 0,17–0,19 ms; Clippy `-D warnings` do `ntm-core` e da glue: 0 |
| U: mutações | 10/11 pegas, 1 equivalente (`evidence/mutations-t07.log`): sem teto no buffer da barra, nada muda, porque a rede nunca entrega mais do que a barra pede (`demand` já é `max − power`). Depois da revisão, um teste direto do `give` (P3 c) pega também esse |
| J: anfitrião e hóspede LAN (`run-rbmk-t05-proof.py --script native_rbmk_t07.py --reopen`, `evidence/j-run5/`) | **23/23 e 1/1 na reabertura**. Pelas telas: manual (anfitrião) e moderada, ReaSim (hóspede) tomam o alvo, andam e alternam o grupo; automática e ReaSim automática tiram o alvo da curva (0,6) e andam. Energia sem célula criativa: as duas ReaSim alimentadas por uma bateria com 1 000 000 HE pela face de baixo chegam aos alvos; paradas não gastam (bateria 1 000 000, buffers 50 000); uma barra andando de 0,578 a 0,501 tira 139 944 HE da bateria (28 ticks a 5000, 0,00277 por tick); de volta ao alvo não gasta (545 000 constante); com a bateria em 0 a barra para (0,170 com alvo 0). Fluxo: barra em 0 zera o que vem atrás; em 1 deixa passar. Reaberto: manual 0,5, moderada 1, automáticas 0,6 e a ReaSim parada em 0,170 com alvo 0 (energia não gravada). Binário `d02db588…`, pacote `0f9c0154…`. As rodadas 2–4 caíram pelo roteiro: leitura atrasada da bateria (o gancho `ntmtest power` lê o servidor), a célula criativa no slot da bateria, e o orçamento da bateria (descer de 0,75 a 0 custa mais do que ela tinha; agora volta a 0,5 e a falta de energia virou a verificação final) |

### Decisões do coordenador (20/11/2026)

Energia das ReaSim não gravada, como no Java: aceito. Ferramenta de cópia: ticket próprio depois do T07 aprovado.
Prioridade LOW da rede: limite registrado, aceito. AZ-5 no T10 e RTTY/OpenComputers no T13 e D7: aceitos.

### P3 da revisão do T07 (20/11/2026)

A revisão independente aprovou o T07 (sem P1/P2) e apontou três P3, um commit cada:

| Item | Como fica | Commit |
| --- | --- | --- |
| (a) prioridade LOW | Registrado em "Desvios e decisões": as ReaSim declaram LOW quando a rede tiver prioridades | `add05e65` |
| (b) energia fora do passo | Registrado em "Desvios e decisões": a checagem de 5000 lê o buffer do tick anterior | `b6825301` |
| (c) mutante equivalente | O teto do `give` fica (é o `transferPower` do Java: o excedente volta) e ganhou o teste direto `a_reasim_rods_buffer_stops_at_its_maximum`, que pega a mutação "sem teto" | `e44b53a7` |

## RBMK T08 — caldeira, aquecedor, resfriador e fluido ReaSim (20/11/2026)

Branch `rbmk-t05` (continua sobre o T07), motor sem mudança. A física das três colunas já roda no passo desde o T04
(oráculo `run_flux.py`), mas com os tanques secos: o save não os guarda, nenhuma rede chega a eles e o estado em memória
(atraso do respiro, temporizador do resfriador) volta ao do TE novo a cada tick.

### Plano

| # | Parte | O quê | Fonte Java |
| --- | --- | --- | --- |
| N1 | Paridade | `FluidTank.setTankType` esvazia o tanque quando o tipo muda. O stub do oráculo não fazia isso, e o port copiou o erro: o compressor da caldeira esvazia o vapor (o `/10` e o `*1000` agem sobre 0), o aquecedor esvazia o tanque quente quando o tipo produzido muda, e os dois tanques ao virarem `NONE`. Stub corrigido com o corpo real, fixture regerada, núcleo corrigido | `FluidTank.setTankType:62-68`, `cyceCompressor`, `TileEntityRBMKHeater.updateEntity` |
| N2 | Estado | Tanques gravados como o `writeToNBT` (caldeira `feed`/`steam`, aquecedor `feed`/`steam`, resfriador `t0`/`t1`); só em memória o que o Java não grava: `ventDelay`, consumo e saída da caldeira, o temporizador do resfriador e o seu cache 5×5 (por posição e geração: uma coluna trocada dentro dos 60 ticks não é resfriada, como o TE morto do Java) | `readFromNBT/writeToNBT` das três, `neighborCache` |
| N3 | Nós de rede | Cada coluna é nó das redes dos seus dois tipos (entrada e saída); entrada e saída ReaSim (`rbmk_steam_inlet`, água 32 000; `rbmk_steam_outlet`, vapor superaquecido 32 000) são máquinas com porta em volta e passam água/vapor às colunas ao lado com `dialReasimBoilers` | `ModulePortManFluidAdaptive`, `TileEntityRBMKInlet/Outlet` |
| N4 | Portas | As portas da coluna por `column_ports` (sem loader, loader a 1, loader a 2) num grupo de passagem: os canos no topo e embaixo (ou em volta do loader) são uma rede só para cada tipo, como o `PortDef.combine` com um nó de várias posições | `TilePort`, `getPortsNoLoader/CloseLoader/ClassicLoader` |
| S1 | Servidor | Tanques e estado em memória entram no passo e voltam com o commit; o identificador do aquecedor vem do slot; o respiro da caldeira (`Effect::SteamVent`) toca `hbm:block.steamEngineOperate` e solta a partícula `rbmksteam` | `updateEntity` das três |
| S2 | Comandos e telas | O compressor como o botão 0 do menu (as telas do conteúdo já o declaravam; cada clique na sua vez compõe como os pacotes do Java); os valores dos tanques para as telas da caldeira e do aquecedor | `TileEntityRBMKBoiler.receiveControl`, `GUIRBMKBoiler`, `GUIRBMKHeater` |
| G | Glue e conteúdo | Telas com os medidores e o botão do compressor; loader, entrada e saída como blocos com portas | `GUIRBMKBoiler`, `GUIRBMKHeater` |
| U | Provas | Oráculo regerado (cenários novos: compressor com vapor, troca de tipo do aquecedor, `NONE`), unidade de portas e passagem, mutantes | — |
| J/C | Em jogo | Circuito de água e vapor pela caldeira (sem loader, loader a 1 e a 2), criogel pelo resfriador, refrigerante pelo aquecedor, entrada e saída ReaSim; água consumida, vapor entregue e ventado e criogel devolvido contados antes/depois, e depois de uma reabertura | — |

### O que mudou

| Parte | Comportamento | Fonte Java |
| --- | --- | --- |
| Tanques (N1) | `FluidTank.setTankType` esvazia o tanque quando o tipo muda: o compressor perde todo o vapor, o aquecedor esvazia o tanque quente quando o tipo produzido muda e os dois tanques ao virarem `NONE`. O stub do oráculo tinha o mesmo erro; corrigido com o corpo real e a fixture regerada (só os cenários do compressor e do aquecedor mudaram) | `FluidTank.setTankType:62-68` |
| Estado (N2) | `PipedState` na coluna: os dois tanques como o `writeToNBT` (tipo, quantidade, máximo); só em memória o atraso do respiro, o consumo e a saída da caldeira, o temporizador do resfriador e o seu cache 5×5 (por célula e registro). O identificador no slot do aquecedor tipa a entrada. Um resfriador liga ao seu reator as colunas do seu 5×5, encostadas ou não | `readFromNBT/writeToNBT`, `neighborCache`, `feed.setType(0, slots)` |
| Respiro (S1) | A caldeira cheia toca `hbm:block.steamEngineOperate` (volume 2, tom 1 + rand·0,25) no canto do núcleo, `getColumnHeight` acima, com o commit do passo | `TileEntityRBMKBoiler.updateEntity` |
| Redes (N3) | Caldeira, aquecedor e resfriador são nós das redes dos tipos dos seus dois tanques: recebem no de entrada o que cabe e oferecem todo o de saída | `IFluidStandardTransceiverMK2`, `ModulePortManFluidAdaptive` |
| Portas (N4) | As portas Java (sem loader: topo e base; loader a 1: a célula do loader, aberta em volta; loader a 2: base e loader), com a altura do dial; um grupo de passagem: os canos em qualquer porta são uma rede só para cada tipo. O loader é lido do mundo (bloco simples, sem máquina) | `getPortsNoLoader/CloseLoader/ClassicLoader`, `TilePort`, `PortDef.combine` |
| Entrada/saída ReaSim (N3) | `rbmk_steam_inlet` (água, 32 000) e `rbmk_steam_outlet` (vapor superaquecido, 32 000) viraram máquinas com porta em toda face; com `dialReasimBoilers`, depois das redes, enchem a água ReaSim das colunas dos lados (qualquer célula, `findCore`) até 16 000 ou tiram o vapor delas | `TileEntityRBMKInlet/Outlet.updateEntity` |
| Telas (S2) | Caldeira: tanques, grau do vapor (0..3) e o botão 0 do compressor (cada clique na sua vez); aquecedor: tanques e cores. A caldeira (sem slot) ganhou menu, senão a tela não abria | `GUIRBMKBoiler`, `GUIRBMKHeater`, `ContainerRBMKGeneric` |
| Ponte | Uma entidade de bloco lida de volta do save com menos slots que a máquina (itens até o último guardado) completa o resto vazio: um barril com o recipiente vazio no slot 3 derrubava o servidor ao reabrir (achado da prova J) | — |
| Gancho | `ntmtest heat x y z calor` (só com `NTM_TEST_HOOKS=1`): o próximo passo da coluna começa nesse calor | — |

### Desvios e decisões para o coordenador

- **Fluido fora do passo** (como a energia do T07): as redes e a entrada/saída ReaSim mexem nos tanques fora do passo do
  reator; a coluna anda com o que o tick anterior deixou. No Java, rede e `updateEntity` correm na ordem dos TEs.
- **Rede refeita a cada 10 ticks** (limite geral da rede do port): um loader posto ou tirado muda as portas na checagem
  seguinte; no Java o `TilePort` refaz no mesmo tick.
- **Altura das portas pelo dial** (`getColumnHeight`, como o Java lê), não pela altura da coluna em pé (D5): uma coluna
  de outra altura tem a porta de cima onde o dial diz. Mesmo critério da tampa no T02.
- **Passagem só entre canos**: dois canos em portas diferentes viram uma rede; uma máquina encostada numa porta (barril
  no topo) faz par só com a coluna, não entra na rede dos canos.
- **`canConnect` do loader não é regra de fluxo** (conferido no Java, decisão do coordenador): o elo de rede do cano é
  só entre nós (`UniNodespace.checkNodeConnection`/`checkConnection` comparam as conexões `DirPos`, sem consultar o
  bloco), e o nó da porta fica na célula do loader com as 6 conexões; `RBMKLoader.canConnect` só chega por
  `Library.canConnectFluid` a `FluidDuctStandard.canConnectTo` (caixa de colisão e seleção do cano) e a
  `RenderBoxDuct`/`RenderFluidBarrel` (desenho). Um cano de qualquer tipo ao lado do loader passa fluido, como no Java;
  o desenho e a caixa do cano encostado no loader são do T19.
- **Compressor pelo botão do menu**: precisa da tela aberta; o Java aceita o pacote de qualquer jogador a menos de 20
  blocos (`hasPermission`).
- **Barra de vapor e dicas da caldeira**: a barra é proporcional (o Java soma 1 duas vezes, diferença de 1 px) e as
  dicas dizem "Water"/"Steam" fixos (o Java mostra o nome do fluido); a partícula `rbmksteam` do respiro. T19.
- **Queda (C após crash)**: rodada depois da revisão (ver "Revisão do T08"): nada perdido nem criado nos cinco cortes.

### Validação

Commits: `c5a7e4e6` (N1: tanque retipado esvazia), `6b4ef2dd` (N2: tanques e estado), `c7fc0035` (respiro),
`8f187624` (N3/N4: redes e portas), `849ddd68` (entrada e saída ReaSim), `4f7eb382` (compressor e telas),
`3898731a` (gancho `ntmtest heat`), `c0deb028` (Clippy), `66bae438` (correção: menu da caldeira), `50df5ca2` e
`687e3777` (correção: loader lido do mundo), `92a29f18` (correção da ponte: slots lidos de volta), `c5ee1d85` (executor),
`19fbdb4b` (roteiro J). Evidências em `/mnt/ssd120/sacer/so/rbmk-t08/evidence/`.

| Prova | Resultado |
| --- | --- |
| U: oráculo Java | Stub de `FluidTank.setTankType` corrigido com o corpo real; `run_flux.py` regerado (JDK 25.0.5.1): só `boiler_compressor`, `heater_coolant`, `heater_every_heatable` mudam e entra `heater_retype`; os demais 40 cenários idênticos bit a bit. `rbmk_flux_parity` e `rbmk_oracle_parity` verdes |
| U: unidade | Caldeira ferve e grava os tanques, atraso do respiro em memória e um som só por respiro; resfriador no 5×5 (coluna sem encostar, cache por registro, coluna posta ou trocada depois da leitura); identificador do aquecedor; save anterior ao T08; portas sem loader, loader a 1 e a 2 (lido do mundo), passagem entre canos; entrada e saída ReaSim (dial, teto 16 000, a partir de um dummy, redes, save); compressor (cada clique um grau, o vapor perdido, quente e com água nada, outro botão nada) e telas; menu da caldeira; gancho de calor; barril lido de volta com menos slots |
| U: suíte (`92a29f18`) | `ntm-core` debug 938 ok, 6 ignorados, 0 falhas; `save_model` release 11/11; `keep_world_cost` 0,17–0,18 ms; Clippy `-D warnings` do `ntm-core` (todos os alvos) e da glue (stage do binário `d`): 0 |
| U: mutações | 35/37 pegas, 2 equivalentes (`mutations-*.log`): o cache por registro (uma coluna trocada sempre perde um passo, e o cache perde a célula nele; mantido como defesa) e o padrão de tanques de um save antigo (o `Column::new` já dá os mesmos; o código redundante saiu) |
| J: anfitrião e hóspede LAN (`run-rbmk-t05-proof.py --script native_rbmk_t08.py --reopen`, `evidence/j-run6/`) | **12/12 e 1/1 na reabertura**. Caldeira sem loader: 10 000 de água pela base, 6 000 ficam no barril; aquecida, 6 000 mB de água viram 600 000 mB de vapor (584 000 no tanque, 16 000 no barril pelo topo). O hóspede aperta o compressor duas vezes: STEAM → HOTSTEAM → SUPERHOTSTEAM, o vapor perdido. Loader a 1 e a 2: 10 000 pela célula do loader. Resfriador: a coluna a duas casas vai de 900 a 20; 16 000 mB de criogel frio saem usados pelo topo (soma 16 000). Aquecedor: 2 664 mB de refrigerante quente pelo topo (soma 16 000). ReaSim: a entrada ao lado do dummy enche a água da coluna (16 000); fervida, o vapor sai pela saída (15 981 no barril, 19 na coluna, soma 16 000). Em repouso antes do save; reaberto, todos os tanques e barris iguais. Binário `7b38f01a…`, pacote `0f9c0154…` (o do T07; o conteúdo não mudou). As rodadas 1–5 caíram por: o roteiro (piso para o hóspede; fase que parava as outras), o menu da caldeira (defeito, `66bae438`), o loader procurado entre as máquinas (defeito, `50df5ca2`), a espera do executor (`c5ee1d85`) e o servidor que caía ao reabrir um barril (defeito da ponte, `92a29f18`) |


### Revisão do T08 (20/11/2026)

A revisão independente aprovou o T08 e os P3 do T07 (sem P1/P2). Decisões do coordenador: os desvios registrados
(fluido fora do passo, rede refeita a cada 10 ticks, altura das portas pelo dial, passagem só entre canos, compressor só
com a tela aberta, barra/dicas/partícula para o T19) aceitos; o `canConnect` do loader não é regra de fluxo (`c5187e95`).

| Item | Como fica | Commit |
| --- | --- | --- |
| P3-1 slots completos na fonte | O motor lê de volta os itens de uma entidade de bloco só até o último guardado (`decode_mod_block_entity`: "the game sizes them to the block's container when it uses them"). A ponte completava só a sua cópia; os outros leitores de `machine_slots` (`rbmk_server` 661/705, `rbmk_columns` 366/398, `solar_server` 91, `bomb_server` 528/550, `pile_server` 315, `native_server` 136/504/514, `transaction` 623/847, `zirnox_server` 81) recebiam o vetor curto, e todo commit comparava o esperado com os itens curtos: um canal gravado vazio recusava toda barra posta à mão depois de reabrir. Agora o acesso da glue à entidade (`entity`) completa os itens (`native::size_slots`) antes de qualquer leitor ou commit; o host de teste faz igual. Sem mudança de motor. Testes: barril com 4 de 6 slots, canal com 0 de 1 slot que ainda aceita a barra à mão; mutante pego pelos dois. Suíte em `893e019b`: `ntm-core` debug 940 ok, 0 falhas; `save_model` release 11/11; Clippy do núcleo e da glue (binário `b9582404…`): 0 | `4046c940` |
| P3-2 quantidade do compressor | Na J (`evidence/j-run7/`, **13/13 e 1/1 na reabertura**, binário `b9582404…`), a caldeira que ferveu (584 000 mB de vapor, 10 000 de água, fria a 20 °C) tem o compressor apertado pelo hóspede: HOTSTEAM com 0 mB, a água mantida (o `setTankType` esvazia antes do `/ 10`) | `0a54a810` |
| Revisão do complemento: P3-2 primeira leitura no mundo estacionado | No caminho do mundo estacionado (`entity` da glue) os itens são completados antes da cópia `before`: a primeira leitura depois de carregar não conta como `moved` nem chama `container_changed`. No nível em jogo o motor compara em volta de toda a chamada (`native_with_mod_block_entity`): a primeira leitura depois de carregar conta uma vez como mudança (um `container_changed`; nenhum item se move) | `71b89463` |
| Revisão do complemento: P3-3 `cell_snapshot` | Não lê nem recusa à toa: `contents()` de uma entidade de mod é `items.iter().flatten()` (`containers.rs`, `Self::Mod`) e `encode_items` pula os vazios (`item_nbt.rs:95-107`); completar com vazios não muda nem os itens nem o SNBT da fotografia | — |
| P3-3 pontos sem revisão a fundo | O som do respiro (`c7fc0035`) e o menu da caldeira (`66bae438`, `4f7eb382`) não foram lidos a fundo pela revisão: ficam como sem revisão independente, cobertos só pelos testes e pela J deste ticket | — |
| Corte C com fluido em movimento | `run-rbmk-t08-c-proof.py` + `native_rbmk_t08_c.py`: 16 000 mB de criogel saindo de um barril no chunk (1, 0), passando por um resfriador no chunk (2, 0) e voltando a um barril no (1, 0); o jogo morto em `kill`, `kv_committed`, `region_record:1`, `chunks_written` e `world_saved` com o fluido em movimento (no save: 6 300–6 800 na fonte, 4 000 no resfriador, 5 200–5 700 no destino). Nas duas reaberturas, a fonte, os dois tanques do resfriador e o destino, lidos de uma vez (`ntmtest fluids`, `b00d69e2`), somam **16 000 mB em todos os pontos**: nos 5 cortes nada se perdeu, mas nenhum caiu entre os registros dos dois chunks: `kill`, `kv_committed` e `region_record:1` voltam inteiros ao save anterior (16 000 na fonte); `chunks_written` e `world_saved`, ao novo. Controle vanilla no mesmo corte: 64 pedras levadas por funis pela mesma divisa, 64/64 em todos os pontos. **Limite geral PROVADO** na varredura seguinte (`evidence/c-run5/`): nos cortes `region_record:2` e `region_record:3`, que caem entre os registros dos dois chunks, a reabertura soma **20 000 mB**: a fonte (chunk (1, 0)) volta do save anterior com os 16 000 inteiros e o resfriador (chunk (2, 0)) do save novo com os 4 000 que já tinha recebido, que passam a existir duas vezes (`region_record:4` volta ao save novo inteiro). Causa: as redes de fluido mexem nas máquinas fora das transações, e o save grava os chunks em registros separados; vale para qualquer máquina do port ligada por cano entre dois chunks (o gás do extrator do T09 incluído). Aceito temporariamente como limite declarado (decisão do coordenador, levada ao usuário), com o ticket de infraestrutura "Rede de fluido e save atômicos entre chunks" no plano, prioridade alta. Comparação com o vanilla (`evidence/c-run6-vanilla/`, só o controle, `region_record:1..6`): o vanilla também duplica quando o corte cai entre os dois chunks: em `region_record:2` e `:3` reabre com **79 e 78 pedras** em vez de 64 (o baú do chunk (1, 0) do save anterior com as 64, o do (2, 0) do novo com as 14–15 que já tinham passado), na mesma proporção (~23 % a mais no vanilla, ~25 % no port). O que está provado é só isso, qualitativo: a mesma classe de falha. Que os dois rasguem "nas mesmas emendas" não está provado: a numeração `region_record:N` não é uma sequência estável do mesmo save (cada ponto é outra rodada, com outro estado e outra ordem de registros), e o "não é pior que o vanilla" vale só como "a mesma classe de falha". Conforme o lado gravado, o rasgo duplica ou perde. O ticket INFRA-F continua: as regras de frentes pedem nenhuma perda nem duplicação, não "igual ao vanilla". Binário `b9582404…`, pacote `0f9c0154…`; `evidence/c-run3/` (`kill`, `kv_committed`) e `evidence/c-run4/` (os outros três). As rodadas 1–2 e o `world_saved` da 3 caíram pelo roteiro (funil que puxa de cima; contagem vanilla em movimento; leitura depois do corte) | `b00d69e2`, `052ae523` |

## RBMK T09 — extrator de gases (outgasser) por fluxo (20/11/2026)

Branch `rbmk-t05` (continua sobre o T08), motor sem mudança. A física já roda no passo desde o T04 (oráculo:
`outgasser_recipes`, limiares `> 10000`), mas sem mundo: os slots não são lidos ("o outgasser que não processa"), o
progresso e o tanque de gás não são gravados, o gás não sai pelas portas, a automação recusa a entrada e a tela não lê
nada.

### Plano

| # | Parte | O quê | Fonte Java |
| --- | --- | --- | --- |
| N1 | Itens das receitas | As 19 receitas `OutgasserRecipes.registerDefaults` (oráculo do T00) ligadas aos ids do port: entradas por OreDict pelos nomes do item do port (`oredict::names`, na ordem do OreDict), entradas por item e saídas por um mapa Java (item, meta) → id do port conferido contra o `ids.json` (a meta pela ordem do enum Java, como `oil_tar` meta 2 = `EnumTarType.COAL` → `ntm:oil_tar__coal`); um item sem par vira recusa nomeada, não um item inventado | `OutgasserRecipes`, `ItemEnums` |
| N2 | Estado | `progress` (pelos bits) e o tanque `gas` (tipo, quantidade, máximo 64 000) gravados como o `writeToNBT`; os dois slots são os do motor (entrada 0, saída 1) e entram no passo e voltam com o commit, os dados de stack preservados | `readFromNBT/writeToNBT`, `TileEntityRBMKSlottedBase` |
| N3 | Gás nas portas | O extrator é nó da rede do tipo do seu tanque (só oferece), pelas mesmas portas da coluna com loader do T08 | `ModulePortManFluidAdaptive.setOutputTanks(gas)` |
| N4 | Automação | Funil insere só no slot 0 e só itens com receita (`isItemValidForSlot`), retira só do 1 (`canExtractItem`); a mão segue a regra do T05 | `isItemValidForSlot`, `canExtractItem`, `getAccessibleSlotsFromSide` |
| S | Tela | Barra de progresso e medidor de gás do `horizon_screen/rbmk_outgasser.json` com os valores do servidor | `GUIRBMKOutgasser` |
| U | Provas | Oráculo do T04 (já cobre `>` em 10000, perda do excedente, rápido/lento, reset, `fusionOnly`, saída cheia); unidade das ligações de item, do estado, da porta e da automação; mutantes | — |
| J/C | Em jogo | Insumo posto por funil, irradiado por uma barra no reator, sólido retirado por funil e gás por cano até um barril; reabertura; corte com o processamento em andamento: exatamente um consumo e as saídas de uma receita por processamento, nada refeito depois da queda | — |

### O que mudou

| Parte | Comportamento | Fonte Java |
| --- | --- | --- |
| Itens (N1) | `outgasser::PORT_ITEMS`: os 13 itens Java que as receitas nomeiam ↔ ids do port (a meta do `oil_tar` pelo `EnumTarType`: 2 `ntm:oil_tar__coal`, 4 `ntm:oil_tar__wax`; os vanilla do 1.7.10; o ensopado brilhante empilha 1, `ItemSoup`); as entradas por OreDict pelos nomes do item do port (`oredict::names`). O cogumelo brilhante (`ModBlocks.mush`) não está no conteúdo: as duas receitas de cogumelo não rodam (`outgasser::MISSING`, decisão do coordenador; ticket T09-M), e um teste avisa quando o bloco entrar no `ids.json` | `OutgasserRecipes.registerDefaults`, `ItemEnums` |
| Estado (N2) | `persist::OutgasserState`: `progress` pelos bits e o tanque `gas` (tipo, quantidade, 64 000) gravados; os dois slots são os do motor: entram no passo nos termos das receitas e voltam como os itens do port (a pilha da entrada, menor; a da saída, maior, ou o item do produto num slot vazio) | `readFromNBT/writeToNBT`, `canProcess`, `process` |
| Gás (N3) | O extrator oferece o gás na rede do tipo do seu tanque, pelas portas da coluna com loader (as do T08, `persist::has_column_ports`) | `setOutputTanks(gas)` |
| Automação (N4) | Funil põe só no slot 0 e só o que tem receita; retira só do 1 | `isItemValidForSlot`, `canExtractItem` |
| Tela (S) | A seta `(int)(progress * 13 / 10000)` de 13 pixels e o gás (quantidade, máximo) no `horizon_screen/rbmk_outgasser.json` | `GUIRBMKOutgasser` |
| Gancho | `ntmtest fluids` lê também o gás e os slots do extrator, na mesma leitura | — |

### Desvios e limites

- **Cogumelos sem receita** até o T09-M (o produto, `ModBlocks.mush`, não existe no port): o funil não os põe e o
  progresso fica em 0; no Java viram cogumelo brilhante.
- **Sólido só pela tela**: o inventário da coluna fica só na célula de cima (como no Java e no T02), e um funil só puxa
  do bloco acima dele: nada retira o sólido por automação comum; o jogador tira pela tela.
- **Gás pela rede**: o mesmo limite geral provado no T08 (corte entre os registros de dois chunks ligados por cano
  pode duplicar ou perder fluido; ticket INFRA-F no plano). Nesta topologia (extrator no chunk (2, 0), barril do gás
  no (1, 0)), se o (2, 0) chega novo (piches consumidos) e o (1, 0) antigo (sem o gás que já tinha passado), somem
  100 mB por piche processado.
- **Pilha do ensopado brilhante**: o conteúdo deixa o `ntm:glowing_stew` empilhar 64 (sem `max_stack_size`); no Java
  é um `ItemSoup`, que empilha 1. O extrator usa o 1 do Java (só faz outro com a saída vazia); o teste
  `stack_limits_are_the_contents_but_its_known_gap` nomeia a lacuna do conteúdo e avisa quando ela mudar.
- **Ordem do OreDict**: o `getOutput` do Java percorre os nomes do item na ordem de registro do OreDict; aqui na ordem
  dos nomes do port. Só diferiria para um item com dois nomes de receitas diferentes: nenhum no port (teste
  `no_port_item_has_two_recipe_names`).

### Validação

Commits: `d87bef17` (N1: itens das receitas), `983de48d` (N4: funis), `c270f723` (N2/N3: slots, progresso, gás e
portas), `7522f8bd` (tela), `5cac2c0e` (gancho `ntmtest fluids`), `3d07496f` (ordem do OreDict), `b7fa3f96` (doc e
roteiro J), `cb2d4b97` (roteiro C). Evidências em `/mnt/ssd120/sacer/so/rbmk-t09/evidence/`.

| Prova | Resultado |
| --- | --- |
| U: oráculo Java | O do T04 (`outgasser_recipes`, limiares `> 10000`, perda do excedente, rápido/lento, reset, `fusionOnly`, saída cheia), verde |
| U: unidade | Itens das receitas (cada item Java do port ou lacuna nomeada; ids no `ids.json`; receitas pelo item e pelo OreDict; cogumelo sem receita; nenhum item com dois nomes de receitas); piche irradiado vira óleo de carvão e o save guarda gás e progresso; ouro vira o lingote Au-198 do port no slot de saída; cogumelo espera (progresso 0) e funil só põe o que tem receita no slot 0; gás sai pela porta; tela (seta truncada, gás); gancho lê gás e slots juntos |
| U: suíte | `ntm-core` debug 949 ok, 6 ignorados, 0 falhas; Clippy `-D warnings` do `ntm-core` e da glue (binário `d30579da`): 0; `save_model` release 11/11; `keep_world_cost` 0,18 / 0,18 / 0,23 ms por tick (0, 5 000 e 50 000 gerações; medido com jogos e builds rodando juntos: a última acima dos 0,17–0,19 de antes, sem repetição isolada). Em `cb2d4b97` |
| U: mutações | 13/13 pegas (`mutations-t09.log`) |
| J: anfitrião e hóspede LAN (`native_rbmk_t09.py --reopen`, `evidence/j-run5/`) | **7/7 e 1/1 na reabertura**: extrator posto à mão; um funil sobre o topo põe os 2 piches e os 2 lingotes de ouro (a pedra fica no funil); a barra de Po-210/Be ao lado irradia (`dialOutgasserSpeedMod` 10); 200 mB de óleo de carvão chegam ao barril pelo cano; 2 lingotes de Au-198 no slot de saída, que o hóspede tira pela tela; tudo igual depois de reabrir. Binário `d30579da…`, pacote `0f9c0154…`. As rodadas 1–4 caíram pelo roteiro (o item para colocar, a face do cano, a leitura dos itens do 1.21 com `count` antes do `id`) |
| C: corte com o processamento em andamento (`run-rbmk-t08-c-proof.py --script native_rbmk_t09_c.py`, `evidence/c-run1/`) | Extrator com 4 piches no chunk (2, 0), gás por cano a um barril no (1, 0), a barra posta no ato; no save, 2 piches e 200 mB. Em `kill`, `kv_committed`, `region_record:1`, `:2`, `:3`, `chunks_written` e `world_saved`, as duas reaberturas somam **400** (piche a 100): os cinco primeiros voltam ao save anterior (4 piches), os dois últimos ao novo (2 e 200, e o processamento segue até 0 e 400). Cada processamento gastou um piche e fez o óleo de uma receita, e nada foi refeito depois da queda. Nenhum corte deste mundo caiu na emenda dos dois chunks: o gás que atravessa a divisa segue no limite geral (INFRA-F). Revisão do T09 (P3-2): varredura `region_record:4..8` (`evidence/c-run2/`): 4, 5, 6 e 8 voltam inteiros ao save anterior (400); o 7 não chegou ao corte (o roteiro morreu antes do save). Neste mundo nenhum `region_record:1..8` caiu na emenda: o caso de perda do T09 (o chunk do extrator novo, o do barril antigo: −100 mB por piche processado) **não foi provado** em jogo; fica a análise e a classe de falha provada no T08 |


## RBMK T10 — console 15×15, AZ-5 e parada de emergência (entregue para revisão, 20/11/2026)

Branch `rbmk-t05` (continua sobre o T09), motor sem mudança prevista. O console (`rbmk_console`) e a ferramenta de
ligação (`rbmk_tool`) já existem como conteúdo inerte; os sons `block.shutdown` e `block.rbmk_az5_cover` estão no pacote;
não há tela do console no conteúdo.

### Plano

| # | Parte | O quê | Fonte Java |
| --- | --- | --- | --- |
| L | Ligação | A ferramenta guarda o núcleo da coluna clicada (`posX/Y/Z` no stack) e, no console, passa esse alvo (`setTarget`), com a mensagem `.linked`; o alvo gravado no console; resolve no mundo de uso, como o Java (D4) | `ItemRBMKTool.onItemUse`, `TileEntityRBMKConsole.setTarget`, `readFromNBT/writeToNBT` |
| V | Varredura | A cada 10 ticks, as 15×15 colunas em volta do alvo (`rescan`): tipo de console de cada uma (`getConsoleType`) e os seus dados (`getNBTForConsole`: calor, máximo, indicador do guindaste, moderada; barra: nível e grupo; caldeira: água, vapor e tipo; aquecedor; resfriador; extrator: gás e progresso), como projeção do estado confirmado, sem mexer na física | `TileEntityRBMKConsole.rescan`, `getNBTForConsole` das colunas |
| A | AZ-5 | Comando de tela (sessão do T06, `hasPermission` 20 blocos): `level` 0 com `sel_` de cada coluna de controle que o console vê; no servidor só as barras manuais (`TileEntityRBMKControlManual`: as três manuais) recebem `startingLevel = level` e o alvo 0 (as automáticas não); o surto da descida é o do T07. Na tela, a tampa (primeiro clique abre, som `rbmk_az5_cover` 0,5) e o botão (som `shutdown`, 3 s entre apertos no cliente) | `GUIRBMKConsole.mouseClicked` (AZ-5), `TileEntityRBMKConsole.receiveControl` (`level`) |
| C | Comandos | Nível para a seleção (`level` com `sel_`, `clamp(0, 1)`, o campo de texto pela regra do T06), grupo (`assignColor`, `grabEnumSafely`), compressor das caldeiras selecionadas (`compressor`, o do T08), telas (`toggle` do tipo, `id` com as colunas) | `receiveControl` |
| T | Telas | As seis telas (tipo e colunas) e o histórico, a seleção, o mapa 15×15 com as cores e os medidores, desenhados na glue a partir de um snapshot tipado (D6) | `GUIRBMKConsole`, `RBMKScreen` |
| R | Render | O modelo do console com os valores das telas no mundo | `RenderRBMKConsole` |
| U/J/C | Provas | U: os 225 índices nas quatro rotações, agregações vazias e de tipos misturados, a amostragem de 10 ticks, AZ-5 só nas manuais; J: ligar, abrir, selecionar, mudar nível e grupo, AZ-5 pelo hóspede descendo as manuais e não as automáticas, reabrir; C: comandos só mudam alvos elegíveis, abrir a tela não avança física nem copia inventário | — |

Decisões que o T10 pode pedir ao coordenador: a tela do console (não há JSON de conteúdo; desenho próprio na glue, como as telas das barras do T06/T07) e o render das telas no mundo (T19 se não couber).

Decisões do coordenador (20/11/2026): T10 pelo núcleo (ligação pela `rbmk_tool`, varredura 15×15 a cada 10 ticks,
AZ-5 só nas três manuais, 3 s entre apertos); a tela do console desenhada na glue, aceita; o render das telas no
mundo, se não couber, vai para o T19.

### O que mudou

| Parte | Comportamento | Fonte Java |
| --- | --- | --- |
| Console (máquina) | `Kind::RbmkConsole`, `Data::RbmkConsole(console::Console)`: alvo, rotação e as seis telas gravados (`writeToNBT`); a última varredura e o histórico de fluxo só em memória, como no Java (`columns` e `fluxBuffer` não são gravados) | `TileEntityRBMKConsole` |
| Ligação (L) | Agachado, a `rbmk_tool` numa coluna grava `posX/Y/Z` do núcleo no `custom_data` do stack e diz `.linked`; no console, com o stack marcado, o alvo entra na fila do console e vale no próximo tick dele (`.set`). Agachado, a chave de fenda gira a janela um quarto (`rotate`) e gasta a ferramenta. As mensagens usam `translate_with_fallback` (o conteúdo não tem as chaves `.linked`/`.set`: sai o inglês do Java) | `ItemRBMKTool.onItemUse`, `setTarget`, `RBMKConsole.onScrew`, `rotate` |
| Varredura (V) | A cada 10 ticks do mundo (antes do passo das colunas, como o `updateEntity` lê o que o tick anterior deixou), as 225 células em volta do alvo, giradas pela rotação: tipo (`getConsoleType`), calor e o `getNBTForConsole` de cada classe (barra: nível e, na manual, grupo; canal: enriquecimento, xenônio, calor do casco e do núcleo, ponto de fusão; caldeira e aquecedor: tanques e tipos; extrator: gás e progresso), `moderated` só nas manuais moderadas; o fluxo é a soma do `lastFluxQuantity` dos canais, `(int)`, no histórico de 60 | `rescan`, `getNBTForConsole`, `isModerated` |
| Snapshot (D6) | A tela recebe a varredura num único `IntArray` (`console::pack`: um cabeçalho por célula e os dados da classe, doubles exatos em duas metades), o histórico e as telas. O limite de chaves do codec subiu de 64 para 512: os comandos do console levam uma chave por coluna escolhida (`sel_j`, `sJ`; o AZ-5 de um reator cheio leva até 225) | `serialize` |
| Comandos (A/C) | `receive` no núcleo: `level` com `sel_` (clamp 0..1, NaN passa como no `clamp_double`), `toggle`, `id` com `sN`, `assignColor` (`grabEnumSafely`) e `compressor` com `cols`. Cada efeito espera na fila da coluna, guardado pela geração dela: nível e grupo só nas três manuais (`TileEntityRBMKControlManual`), compressor só na caldeira; célula vazia, de outro tipo ou descarregada fica intocada. O console só guarda suas telas | `receiveControl` |
| Tela (T) | `console_gui` no núcleo (o mapa, as dicas, os cliques: seleção, "selecionar todas as barras", grupos com botão esquerdo e direito, nível pelo campo com a regra do T06, compressor, telas, a tampa do AZ-5 e os 3 s entre apertos, os sons `gui.button.press` por tom, `rbmk_az5_cover` tom 0,5, `shutdown`), a glue (`console_screen.rs`) só desenha e toca. A textura entra no conteúdo (`horizon_screen/rbmk_console.json`, sem título nem inventário: `"inventory": false`, novo no formato) | `GUIRBMKConsole` |

### Desvios e limites

- **Render no mundo (R) e textos das seis telas** (`RenderRBMKConsole`, `prepareScreenInfo`): vão para o T19, como
  decidido. A tela de interface mostra o tipo de cada tela, que é o que o Java desenha nela; o texto agregado só
  aparece no modelo do mundo.
- **Indicador do guindaste** (`craneIndicator`): o console do Java o lê, mas só o console do guindaste o usa; não há
  guindaste no port, e ele não é varrido.
- **Gráfico de fluxo**: as linhas GL de 2 pixels de tela viram quads de 1 pixel de interface (o mesmo na escala 2). Os
  números do gráfico saem em meia escala, sem sombra, como no Java.
- **Textos em inglês**: as chaves `rbmk.*` do Java não estão nas traduções do conteúdo; as dicas usam o texto do
  `en_US.lang` (com os códigos de cor) e os fluidos o nome do port (`fluid::label`).
- **Relógio do AZ-5**: os 3 s são do cliente (`System.currentTimeMillis`), por tela aberta, como no Java; outra tela ou
  outro jogador não esperam.
- **Hóspede de LAN e célula de dummy** (limite de multibloco, ticket **INFRA-G**, prioridade média, decisão do
  coordenador): o uso de um hóspede numa célula de dummy do console não abre menu nenhum, sem rejeição registrada; pelo
  núcleo abre; o anfitrião abre pelo dummy. Provavelmente vale para todo multibloco do NTM. Evidência: a prova J
  registra a tentativa (`guest_through_the_dummy`: falso nas duas aberturas) e abre pelo núcleo. A investigar no motor
  (congelado até a decisão): o caminho do clique do hóspede até o menu do dummy, comparado com o do anfitrião — o
  `UseItemOn` que o cliente do hóspede manda (posição, face, cursor no contorno do multibloco); no anfitrião,
  `lan_actions` (`may_reach_block`, a checagem do cursor com o `multiblock_outline`, se a rejeição é só um
  `HostEvent::Warn` que não chega ao log) e depois `as_guest` → `use_item` → `use_hand` → `machine_cell_core` →
  `mod_menu_at(core)` → `open_menu`.

Decisões do coordenador na entrega (20/11/2026): o achado do hóspede vira o INFRA-G; os demais limites (render e textos
das telas para o T19, `craneIndicator`, textos em inglês, relógio do AZ-5 por tela aberta) aceitos.

### Validação

Commits: `06ac2d4a` (janela, rotação e comandos, puros), `bd92ac4d` (máquina e varredura), `30db767b` (comandos pelas
filas), `f09cfc82` (ferramenta e chave de fenda no núcleo), `0ff3e42d` (varredura empacotada, 512 chaves), `3c2a2523`
(lógica da tela), `dcb858bb` (conteúdo: a tela), `1731ea86` e `ac621e35` (testes), `bd0398ba` (glue: ferramenta e
chave de fenda), `ddec8b04` (glue: a tela), `447417d4` (glue: a sessão da tela do console, achado da prova J),
`e9282aae`…`dfdced89` (roteiro J). Evidências em `/mnt/ssd120/sacer/so/rbmk-t10/evidence/`.

| Prova | Resultado |
| --- | --- |
| U: unidade | Os 225 índices nas quatro rotações; `receive` (nível com clamp e NaN, telas, grupo, compressor, recusa inteira fora da janela); a varredura a cada 10 ticks e girada; AZ-5 e nível só nas manuais, grupo pleno, compressor só na caldeira; ligação e giro pela fila (cinco giros = um); o canal com barra e o fluxo (soma de `lastFluxQuantity`, `(int)`); empacotamento exato de todo tipo (NaN, −0); um reator cheio cabe no snapshot e um AZ-5 de 225 barras num comando; a tela (partes de cada célula, textos, cliques, tampa e 3 s, gráfico); C: abrir a tela não muda máquina nem mão |
| U: suíte | `ntm-core` debug 971 ok (853 da biblioteca + 118 de integração), 6 ignorados, 0 falhas (`evidence/suite-all.log`); `save_model` release 11/11; `keep_world_cost` 0,165 / 0,180 / 0,179 ms por tick (0, 5 000 e 50 000 gerações); Clippy `-D warnings` do `ntm-core` e da glue (binário `7b4e9c49`): 0 |
| U: mutações | Servidor 8/8 (`mutations-console-server.log`; a cadência de 10 ticks sobreviveu na primeira rodada, o teste passou a olhar o tick da varredura), tela 16/16 (`mutations-console-gui.log`), ligação/empacotamento/vista/fluxo 10/10 (`mutations-console-2.log`; o canal moderado dito moderado sobreviveu, o teste do fluxo ganhou um canal moderado), núcleo puro 9/9 (`mutations-console.log`) |
| J: anfitrião e hóspede LAN (`run-rbmk-t05-proof.py --script native_rbmk_t10.py --reopen`, `evidence/j/rbmk-t05-v73ljwdh/`) | **17/17 e 1/1 na reabertura**. O anfitrião põe o console à mão; agachado, a ferramenta guarda a coluna (`posX/Y/Z` 40/101/14) e o console toma o alvo. O anfitrião abre a tela pelo dummy. O hóspede, pela tela: "selecionar todas" e 100 → as três manuais (não a automática); duas colunas no mapa e 25; grupo verde com o botão direito; a tela 1 em temperatura com as duas colunas (112, 113); a caldeira escolhida vai a HOTSTEAM; 7 pedregulhos no inventário, 7 depois. A automática ganha 0,6 pela tela dela; AZ-5 (tampa, depois o botão): as três manuais a 0 a partir de onde estavam, a automática fica em 0,6, a manual desce. A chave de fenda gira o console (rotação 1). Reaberto: alvo, rotação, tela e alvos das barras como gravados. Capturas da tela em `guest/console-*.png`. Binário `7b4e9c49…`, pacote `4578ba4f…` (com a tela do console) |

As rodadas 1–8 caíram por: o roteiro (o núcleo atrás do dummy; a sessão de teclado do campo), o hóspede que não abre
pelo dummy (acima) e um defeito: a tela do console abria sem sessão nem snapshot (`menu_extra_data` só servia as
barras), desenhava só a textura e não mandava nada (`447417d4`).

## Sexta atualização — 09/11/2026, upstream `a3bf6531e`

Upstream congelado no início: `a3bf6531e07f6292414eda44773d30ab0a47f9db` (09/11/2026 20:14 +01:00), **9.404 commits**
após `fc0210381` (o estudo de 05/11 previa 2.980; o upstream andou mais 6.400 até a fusão). Branch do motor
`upstream-a3bf653` sobre `ntm-current` `a31497f38`: fusão `03704788f` e quatro commits locais (`7d447bcb8` exemplos com
o campo `mod_models`, `95a737da9` testes fundidos com os campos locais de menu, `b7db6d985` interruptor de teste da
gravação eager, `a8dc3409f` um `if` dobrado que o Clippy da cópia transformada pedia). Branch NTM `ntm-upstream-a3bf653` sobre `main` `0e8eda47`. `SKIP_UPSTREAM_CHECK=1` só para este
snapshot. Pins: motor `a8dc3409f`, horizon-mixins `5a64af938` (o pin do launcher oficial; antes `ddd9700`), Recipes
`d16065094` (main do horizon-mods; o bundle Recipes não mudou entre os dois pins, só a compatibilidade).

A fusão teve 21 arquivos e ~67 trechos em conflito. O upstream dividiu `game.rs` em muitos módulos (`use_hand`,
`saving`, `autosave`, `inventory_slots`, `menu_clicks`, `creative_screen`, `world_update`, `loading_window`...) e
moveu os testes de `hub.rs` e `native_hooks.rs` para arquivos próprios. Cada mudança local que estava num trecho movido
foi reaplicada no destino; uma verificação linha a linha das adições locais (`fc0210381..a31497f38`) contra a árvore
fundida só aponta as substituições intencionais da tabela abaixo.

| Extensão local | Decisão | Motivo e composição com o oficial |
| --- | --- | --- |
| `Ticking::External` (loops de máquina do NTM) | **Substituída** pelo oficial `Ticking::Native` (#8924) | Mesmo contrato (o dono atualiza ganho/pitch/posição; stop e release explícitos); a variante local foi removida e o teste local (categorias + duas instâncias) foi adaptado a `Native`. NTM: `sounds.rs` usa `Native` e `random_source::create` (o `SoundEngine::unseeded` saiu do upstream). |
| Leitor único de conteúdo (`content.rs`) | **Substituído** pelo oficial (`reader_of`) | O oficial tem a mesma meta (um leitor para todo o índice). Mantido só `door_sounds`, sem equivalente. |
| `recursion_limit`/`json!` aninhado (`lib.rs`, `creatures.rs`) | **Substituído** pelo oficial (#8169) | O upstream adotou a mesma solução. |
| Trouxa: `Slot.allowModification` (`lan/menu.rs`) | **Oficial** | Código idêntico ao local; ficou o comentário oficial. |
| Troca (teclas 1–9/F) com divisão por `slot_max` e slot desocupado | Mantida | O oficial acrescentou só `mayPickup` no slot clicado, que a versão local já exige. |
| `container_accepts` (#9225) × política de slot (`ModSlotFilter::policy`) | Mantida; as duas compõem | O oficial só recusa inserção de funil/dropper/`native_insert_item`, no servidor; não cobre extração, origem manual/automação nem a previsão de menu. No funil, a inserção parcial em slot de mod (sem perder o excedente) foi mantida e o comportamento vanilla (consome tudo) segue para os demais. |
| `host-bridge` (#9222) × `native_players`/CAS de jogador | Mantidas | Helpers para mods de trait (`dyn Host`); não fazem consulta/ação em lote por UUID nem commit conjunto. |
| `machine-breaking` (#9223) × multibloco | Mantida | É quebra por ferramenta de máquina (útil ao NTM no futuro); não resolve célula→núcleo nem quebra conjunta. |
| `chunk-tickets` (#9226) × descarga com o chunk no lugar | Mantida | Tickets mantêm chunks carregados; não dão acesso ao chunk que sai antes de ser gravado. |
| Identidade de BE × `asleep` oficial | Mantida, composta | `put` renova a identidade só fora de `take`/`put`, e acorda o BE. |
| Contêiner exato × `Kind::Mod` oficial | Mantida, composta | Um tipo de menu declarado usa `Kind::Mod`; sem tipo declarado, `Kind::NativeContainer(n)`. Aplicada em `slots()` e no novo `slot_count()`, que precisam andar juntos. |
| Dados i32 de menu × `extra` oficial (API 401) | Mantidos, compostos | `OpenNativeScreen` tem `wide_data` e `extra`; `open_container` passa `extra`. |
| Modelos OBJ × quadro HDR oficial | Mantidos, adaptados | O pipeline dos modelos desenha no formato do quadro (`frame_format`) com a constante `HDR_WRITE_LIGHT`, como as entidades; sem isso não casaria com o passe. Não recebe a deformação `world_warp` nem as marcas HDR das entidades (limite registrado). |
| Painel de receitas | Mantido | O upstream passou a desenhá-lo depois do item carregado; a ordem local (painel sob o cursor) foi mantida e a chamada duplicada saiu. |
| Demais (multiblocos, valores i64, `native_world_access`, `visit_level`, assets, `use_block`, `screen_event`, Flash, save atômico, hooks de item) | Mantidas | Sem equivalente novo; reaplicadas nos módulos novos. `native_world_access` passa `command=false` ao novo `place_state` (o ramo estrito não o usa). |

### Gravação eager de chunks (nova no upstream) e o save atômico

O upstream trouxe `ChunkMap.saveChunksEagerly` (`Game::save_chunks_eagerly`): a cada tick — no meio dele, antes dos
BEs e do fim de tick do NTM, e também com `/tick freeze` — grava até 20 chunks sujos cujo último save tem mais de
10 s, **sem passar por `save_game_with`**, logo sem o gancho `world_saving`. Isso reabria a classe do N1/R7-2: um
bloco mudado entre ticks (um comando, um tick congelado) podia ter o chunk tomado antes do `native_settle`.

| Extensão local | Razão e composição com o oficial |
| --- | --- |
| Uma passada eager que vá gravar ≥ 1 chunk ouve `native_world_saving(false)` antes de tomar o conjunto sujo (`ModifiedChunks::eager_due`) | Mantém o invariante "nada é tomado antes do settle" sem divergir do upstream no autosave. Custo só nos ticks com chunk vencido. Decisão do coordenador (opção A). Testes: `an_eager_pass_is_heard_before_it_takes_a_chunk` (falha se a chamada sumir ou mudar de lugar) e `eager_due_answers_for_the_pass_that_follows`. Candidata a PR upstream junto com `world_saving`. |
| `HORIZON_NO_EAGER_SAVE` (só teste; desligado por padrão) | Desliga a gravação eager nos dois mundos das provas, reproduzindo as condições exatas do R7c. |

A passada eager grava só regiões, não os arquivos de entidades. Esse limite não torna o gancho indistinguível:
`an_eager_pass_is_heard_by_the_mods_only_when_it_saves` (`7cba121ec`) usa um Game real e um mod contador, e distingue
as passadas com e sem gravação. O rastreador verifica a ordem antes da primeira gravação; os mutantes confirmam a
sensibilidade. Os cenários de morte com eager ligado continuam sendo evidência limitada, não provas estritas.

**Mudança de critério do oráculo das provas** (commit isolado `81b10729`, para revisão independente). O critério
"morte fora de um save = exatamente o último save" pressupunha que nada fosse gravado entre saves. O motor novo, como o
vanilla 1.21+, grava; e o momento da passada difere entre o mundo do mod e o controle (o chunk de uma máquina fica sujo
a cada tick, o de um baú só quando muda). Agora, num ponto `kill`/`kv_committed`: se nenhum mundo gravou região ou
entidades fora do save, vale o critério original, inalterado; se gravou, o ponto é julgado item a item por (L) nada
some que o controle guarde, (D) nenhum item com mais cópias que na base ou no controle, as duas reaberturas iguais aos
arquivos e o armazenamento sem obrigações. L e D não foram relaxados. Cada resultado registra o critério usado, o
motivo e se o mod ficou melhor que o controle.

### Adaptações do NTM (branch `ntm-upstream-a3bf653`)

| Commit | Mudança |
| --- | --- |
| `fe42e7d6` | Pins; `sounds.rs` usa `Ticking::Native` e `random_source::create`; itens derramados usam `spawn_item_own` (o item tira o próprio movimento do seu random, como o vanilla; `dispenser::new_item_motion` saiu do upstream); `armor_table.rs` passa os campos novos de `Env` e o `extra` de `open_container`; o céu do End é amostrado por `Sample`. |
| `96c982f6` | Pin do motor com o interruptor de teste. |
| `8db857f6` | `native_stations.py`: o misturador é colocado de um bloco ao lado; do ponto antigo o raio até o chão passava exatamente pela aresta do duto, e o raio do motor novo conta a aresta como acerto. |
| `81b10729` | Critério do oráculo (acima), isolado para revisão. |
| `650133d0` | O glue pergunta as teclas Shift por `HeldKeys::raw` (o upstream proíbe `contains`, que era só um apelido; o Java usa `isShiftKeyDown`, as teclas físicas) e o pin vai a `a8dc3409f`. |

Para o revisor independente, os dois commits de roteiro ficam isolados: `81b10729` (critério do oráculo) e
`8db857f6` (geometria do `stations`).

Nenhuma mudança no `ntm-core`. Os dez scripts locais do usuário continuam byte a byte iguais e fora dos commits.

### Verificação

As provas e medições rodaram no release `horizon-client` SHA-256
`45e273b01484bbaa23a53cfc28b7d5dca9e87470c45848d6f9379dfd7ed131b8` (motor `b7db6d985`, mixins `5a64af938`, Recipes
`d16065094`). O release final, nos pins finais (motor `a8dc3409f`, NTM `650133d0`; as duas mudanças não alteram
comportamento), é `26c81d42f8a1a513484a2f21f093dc49b9ea0ba95f18936deb78404d7c5823c6`. A bateria de confirmação nele **não
passou inteira**: sob carga média de 30 a 100 (CPUs divididas com as outras frentes), items, dims, bombs e pile deram
rc=1 na primeira rodada paralela; em série, items e dims de novo; o controle do `frozen-break kv_committed` estrito não
completou a ação (`player/chunks not ready`). Uma por vez com `NTM_GAME_WIDE=1`: dims, bombs, stations, mining e o
`frozen-break kv_committed` passaram; items, pile e zirnox falharam, cada execução num check diferente, e o **R7c
falhou nos três** nas mesmas condições, alternado com o novo (items no mesmo `thrown_container_waits_its_pickup_delay`,
pile no parser de status, zirnox no enchimento do tanque). Classificação: tempo/carga, nos dois binários; não regressão.
A seção "Correções da revisão" registra a bateria refeita. Pacote `dist/ntm.hmod` `9ab18eac…3cee0aa`, reempacotado com o
`cargo-horizon-mod` desse release e idêntico byte a byte ao do R7c (o conteúdo não mudou).

| Grupo | Resultado |
| --- | --- |
| Motor, `horizon-client-lib` | `lan::menu` 30, `menu::tests` 67, `lan::hub` 33, `lan::containers` 4, `block_entities` 3, `save::` 84, `world::` 3, `native_` 26, `mod_` 83, som 49, `conditional_policy` 4; `lan_bot` 22. |
| Motor, `horizon-client` | `native_` 233, `mod_` 37, `machines` 27, som 16, `world::` 7, `lan::` 13, `menu::tests` 17; ordem do save (`a_save_is_heard_before_it_takes_anything`, `an_eager_pass_is_heard_before_it_takes_a_chunk`) e telas de mod; `mod_api_hooks` 17. |
| Motor, render/mods/world | render 2.075 de 2.080 (as 5 falhas estão em arquivos sem diff local: duas de pilha de crash, que exigem debuginfo, desligado para caber no disco, e três de GPU por software) + 72 dos módulos locais (identidade, `eager_due`, som, modelos); mods 41; world 126 (multiblocos, updates pós-commit, nível). |
| Clippy | `-D warnings` em `horizon-client-render`, `horizon-client-lib`, `horizon-client`, `horizon-mods`, `horizon-world` (lib e testes): limpo. Cópia transformada pelos mixins (motor `a8dc3409f` + bundles NTM e Recipes), `horizon-client-lib` e `horizon-client`: limpo, sem isenções, depois de `a8dc3409f` e `650133d0`. `rustfmt --check` limpo. |
| NTM | 559 testes em debug (512 unitários + 47 de integração) e o verificador do save (`save_model`, 11) em release. |
| Build | Release do cliente com o bundle NTM e o Recipes: ok (9m39s). |

**Regressão em jogo** (mesmo binário e pacote, mundos descartáveis): items, zirnox, inventory-proxy, radioisotopes,
bombs, pile, pile-meltdown, dims, mining, solar, combustion, catalog, comparator, stations, recipes, sounds e
rbmk-t00 passaram. Na primeira rodada paralela (4 jogos + compilações) pile e items falharam e passaram na
reexecução; items teve uma vez um pânico de teardown do wgpu (`SurfaceId` ao derrubar o renderer GL/llvmpipe no
fechamento), que não se repetiu. stations passou depois da correção do roteiro. **audio-lifecycle**: ver pendências.

**Provas do save, modo estrito** (`HORIZON_NO_EAGER_SAVE=1` nos dois mundos, critério original intacto):
`original` 8/8 (`kill`, `kv_committed`, `live_chunk:2,0`/`3,0`, `region_record:1`/`2` com pareamento de cortes,
`entities_written`, `players_saved`), `frozen-unload` 3/3, `frozen-break` 8/9. O nono, `region_record:1` (ponto
acrescentado nesta rodada, fora do conjunto padrão do cenário), é um corte por índice que caiu em chunks diferentes:
o NTM não gravou o chunk das hastes (12/12), o controle gravou (2/2); sem perda nem duplicação, mas o roteiro de
`frozen-break` não tem o pareamento de cortes do `original`.

**Modo eager — evidência, não prova** (comportamento real, critério novo): o resultado L/D depende do instante em que a
passada de cada mundo rodou, então **não prova** nada sobre o save; a prova é o modo estrito acima. Observado:
`kill`/`kv_committed` em `original` 2 = 2, `frozen-unload` 12 = 12 e `frozen-break` 12/12 contra 2/12 do controle (a
passada eager pegou o chunk da máquina antes da quebra, ficando em cooldown, e o do baú depois; não é efeito do gancho).
Relatórios por ponto em `crash/eager-reports/*.points.jsonl`. Os cortes dentro do save
(`save_begin`, `live_chunk`, `region_record`, `entities_written`) não são comparáveis com eager ligado: a passada do
controle já gravou o chunk do baú, o `live_chunk` nunca dispara no controle e o índice de registro cai em outros
chunks. Em nenhum deles o NTM teve menos itens que o controle ou mais que a base. Esses cortes ficam provados no modo
estrito.

Evidências em `.agent-tmp/upstream-update/` (relatórios em `crash/strict-reports`, `crash/eager-reports`, logs em
`crash/`, `regress/`, `perf/`). Mundos aprovados apagados após guardar os relatórios.

**Custo do gancho na passada eager — meltdown** (`pile-meltdown`, `HORIZON_SPIKES`, pior tick por rodada em ms). A
janela quieta foi cancelada (todas as frentes em paralelo); para os dois binários verem o mesmo ruído, cada par rodou
**ao mesmo tempo**, R7c (`98a85002…`, motor `a31497f38`) e o novo (`45e273b0…`), com o mesmo pacote. Carga média de 1
minuto entre 22 e 29 em todas as rodadas.

| Execução | Rodadas (pior tick) | Carga início → fim |
| --- | --- | --- |
| R7c, par 1 | 540/109/50/55/67/72 | 28,6 → 24,1 |
| novo, par 1 | 2/29/42/100/16/49 | 28,6 → 23,5 |
| R7c, par 2 | 23/57/86/43/51/44 | 23,5 → 22,1 |
| novo, par 2 | 21/48/93/27/51/78 | 23,5 → 23,9 |

Doze rodadas por binário: pior tick mediana **45 ms** (novo) contra **56 ms** (R7c), máximo 100 contra 540; média da
janela de pico 18 contra 19 ms. Nenhum cresce por rodada. O novo não é pior que o R7c fora do ruído. Os números
absolutos não se comparam aos do R7c medidos com as frentes paradas (até 32 ms).

Limite desta medição: os dois binários rodaram em pares simultâneos, disputando as mesmas CPUs entre si e com as
outras frentes, sob carga média de 22 a 29. Isso iguala o ruído entre eles e mostra que o novo não é pior, mas não
mede o custo absoluto do gancho; a mediana de 6 rodadas por execução e o máximo isolado (540 ms na primeira rodada do
R7c) são sensíveis a picos de outras tarefas. O critério do R7c (pior tick < ~35 ms com as frentes paradas) fica por
medir numa janela quieta.

**`audio-lifecycle`**: alternando os dois binários três vezes cada sob a mesma carga (24–54), **R7c 0/3 e novo 0/3**.
As falhas do novo foram `arc_lid_slot_creates_channel` (o canal da tampa é criado e parado antes de o mixer de
diagnóstico tocar uma amostra), `active_pitch_changes_on_same_channel` e `message_receipt_precedes_nuclear_playback`; as
do R7c, a tampa. Dois desses checks tinham erro do roteiro (revisor, P3-1): o log nuclear era lido antes de o som tocar,
e a rampa do pitch (0,51 → 1,0 em 25 ticks) já podia estar no topo na primeira amostra. Corrigidos em `99eef2b5`.

### Correções da revisão independente (`3b7a7174`, reprovado → correções)

Branch do motor `upstream-a3bf653-fix` sobre `a8dc3409f`, em commits atômicos:

| Achado | Commit | Correção e prova |
| --- | --- | --- |
| **P1-1** `/forceload add` descarregava colunas sem gravar e sem o gancho (código novo do upstream) | `d93271e38` | O tratamento das colunas descarregadas do quadro virou `retire_unloaded` (gancho E2, LOD, entidades, BEs, `save_later` das sujas), chamado também pela espera do `/forceload`. Evidência original completa em `confirm/probe-bug-{5,6,7,8}.out`: bug-5 **sem resultado**, timeout ao carregar `(402,100,0)`; bug-6 **conservou** as duas pilhas (o log lê a mesma sessão, sem prova de reabertura); bug-7 e bug-8 **perderam os blocos inteiros**, bateria NTM e baú vanilla, depois de salvar/fechar/reabrir. Portanto, quatro tentativas: duas perdas, uma conservação e uma inconclusiva; não quatro perdas nem reprodução determinística. A sonda depende de a varredura cair dentro da espera. `probe-fix-1`/`-2` conservaram; o release posterior `1d8d074b`, #26, também conservou. A re-revisão independente acrescentou duas perdas sem correção e duas conservações com correção, mais uma falha de abertura de cada lado; sua contagem agregada de completas foi 4/5 perdas sem a correção e 4/4 conservações com ela (inclui a bug-6, de protocolo diferente). Essas sondas são evidência histórica; a guarda de regressão deve ser o teste comportamental determinístico. |
| **P2-1** o gancho eager perguntava diferente da passada | `5ab19be7d` | `ModifiedChunks::save_eagerly_heard`: a própria passada chama o gancho uma vez, logo antes do primeiro chunk que vai gravar, com o mesmo `now`, `available()` (prazo do tick e fila do escritor) e luz. Sem tempo, com a fila cheia, nada vencido ou só colunas sem luz, o gancho não roda; nenhum chunk é gravado sem ele. O que os mods sujam ao ouvi-lo volta ao conjunto sujo. |
| **P2-2** teste só textual | `5ab19be7d`, `7cba121ec` | `an_eager_pass_is_heard_once_before_the_first_chunk_it_saves` (rastreador: ordem, uma vez, e nenhum gancho sem tempo, sem nada vencido ou com coluna sem luz) e `an_eager_pass_is_heard_by_the_mods_only_when_it_saves` (jogo real com um mod contador, processo próprio: ouvido uma vez, sem espera, pela passada que grava; não ouvido sem nada sujo, sem tempo no tick, em cooldown). O teste textual agora lê só código (uma chamada comentada o reprova). Mutantes (cada um compilado e rodado, código restaurado depois; `confirm/mutants.log`): m1 gancho trocado por um no-op e m2 chamada comentada reprovam o teste de jogo e o textual; m3 condição invertida (ouvido quando a passada não tem tempo, não antes de gravar) reprova o teste de jogo e o do rastreador. A linha de base passa nos três. |
| **P3-2** `mod_browser_closed` | `ec056cf3f` | Restaurado no fechamento pela tecla de inventário e no fechamento atrás das opções de depuração; teste `every_creative_close_lets_go_of_the_mod_browser`. |
| **P3-3** contrato de `world_saving` | `4676522e0` | Documentado: uma passada eager o emite com `wait` falso e sem `world_saved` em seguida; um mod não deve contar com o par. |
| **P3-4** `region_record` contava escritas eager | `dce43d298` | Failpoint armado implica sem gravação eager. |
| **P3-8** `t.slots` × tamanho do BE | `7c6f44d08` | `slots()` e `slot_count()` usam os slots do tipo de menu declarado, como a previsão. |
| **P3-1** roteiro do `audio-lifecycle` | NTM `99eef2b5` | Releitura do log nuclear depois do canal; pitch julgado a partir do início do canal registrado pelo mixer. |
| **P3-5** oráculo | NTM `39f983f9` (isolado) | Com eager, cortes iguais exigem igualdade com o controle; o resultado L/D sai marcado `proof: false`. |

Também `621642528` (só formatação de `hub_tests.rs`, que a fusão moveu e o `rustfmt` de `horizon-client-lib` pedia).

Depois da correção, na mesma branch (cabeça `6a79054fa9`, pin do NTM em `0aec3c33`):

| Commit | Origem | O que é |
| --- | --- | --- |
| `10ae4aa43d` | Onda 1, `player-attachments-fix` `b22f1fbef1` | Teste `the_owner_reopened_after_an_autosave_loads_its_live_attachments`: grava o dono por `store_owner`, lê o arquivo de volta e o carrega com `load_owner`, como na reabertura depois de um crash. Adaptado aos fixtures do motor novo (`native_machines::tests::server_game`, `world_spawn_sync_tests::host_for`). |
| `88d6b88a2d` | RBMK, `rbmk-t02-engine` `01e5147ef5` | `Hashed::matches` (o `HashedStack.matches` do vanilla): o clique só concorda com a previsão do cliente se item, quantidade, componentes adicionados (mesmo hash) e removidos forem iguais; componente que o servidor não sabe hashear conta como diferente e o slot é reenviado. `lan/containers.rs` `hashed_matches` passa a usá-lo. Teste `item::hash::tests::a_claim_matches_only_the_same_components`. |
| `6a79054fa9` | RBMK, `rbmk-t02-engine` `93359657aa` | A retirada do slot 200 do próprio anfitrião (`menu_clicks.rs`, laço do `ResultSlot`) passa o resultado por `native_prepare_item_craft` (`host_prepared_craft` em `inventory_slots.rs`): grade 2x2 do inventário (`pos` None) ou da mesa, cada pilha de um shift-click; nada é retirado se o mod deixar o resultado vazio. O slot continua mostrando o resultado vanilla. No upstream o laço saiu de `game.rs` para `menu_clicks.rs`. |

**Verificação no motor corrigido** (`7cba121ec`; `621642528` só muda um arquivo de teste). Release `horizon-client`
`1d8d074b397e8ba9da80c4cc4dcf3ef464f64cbb89e0efccee66a748a273e724`, mesmo pacote `9ab18eac…`.

- Motor: `horizon-client-lib` `native_` 26, `menu::tests` 67, `lan::` 382, `save::` 84, `mod_` 83, som 49,
  `conditional_policy` 4; `horizon-client` `native_` 235, `machines` 27, `mod_` 38, `world::` 7, os testes novos (ordem do
  save, eager, `/forceload`, fechamento do criativo) e o de jogo `an_eager_pass_is_heard_by_the_mods_only_when_it_saves`;
  `lan_bot` 22; render 72 dos módulos locais. Clippy `-D warnings` limpo nas cinco bibliotecas e na cópia transformada;
  `rustfmt` limpo.
- **Provas do save, modo estrito, critério original** (uma por vez, `NTM_GAME_WIDE=1`): `original` **8/8** (`kill`,
  `kv_committed`, `live_chunk:2,0`/`3,0`, `region_record:1`/`2`, `entities_written`, `players_saved`; todos iguais ao
  controle item a item, os cortes por índice agora sem pareamento porque o failpoint desliga a passada eager),
  `frozen-break` **8/8** (`none`, `kill`, `kv_committed`, `save_begin`, `live_chunk:2,0`, `chunks_written`,
  `entities_written`, `players_saved`), `frozen-unload` **3/3**.
- **Bateria limpa, uma por vez, `NTM_GAME_WIDE=1`** (carga média 23–33 das outras frentes): dims, pile, stations,
  `audio-lifecycle` (roteiro corrigido; ciclo de vida, pitch, tampa e trabalho do forno a arco, porta, nuclear e
  fechamento) e **items** passaram inteiros; o `items` limpo é o primeiro no motor novo. O primeiro `items` da fila
  falhou em `real_eating_started` (check de relógio real). Um reinício do host às 02:20 e um travamento da máquina às
  07:14 cortaram filas no meio; o que estava em andamento foi descartado e refeito.
- **`bombs`** (uma por vez, `NTM_GAME_WIDE=1`, carga 20–50): no corrigido 2 passaram inteiros (#28, #39), 1 caiu na
  preparação (`player/chunks not ready`) e 3 (#30, #34, #36) falharam no mesmo check do reinício,
  `the_nukes_crater_stops_with_the_world`. Nos 3, o log da primeira sessão termina com `crater done after 917 ticks` e
  `fallout starts after 939 ticks`, gravados antes do fim do `restart-arm`. A cratera da nuke de raio 80 acabou ainda na
  primeira sessão, e o save do fechamento gravou a explosão já no fallout. Na reabertura ela volta como fallout
  (`EntityFalloutRain` guarda o `scale`; `drop_craters` só descarta a fase `crater`), e o check, que espera "a cratera
  para com o mundo", não vê essa linha. O roteiro supõe que, com `fuel 4`, a cratera dura minutos, mas sob carga ela
  acabou entre o save do `restart-arm` (status `Planning`, 264/1012 setores) e o fechamento. É uma corrida do roteiro,
  sem perda: a balefire voltou e terminou nos três, e o mundo reaberto segue o que foi gravado. Falhas no `final`
  (`26c81d42`): 1 passou (#3); #32 na preparação, #35 `substring not found`, #38 `physical_lever_powered`. No R7c: #33
  `substring not found`. Sem a gravação eager (`HORIZON_NO_EAGER_SAVE`, #40): `physical_lever_powered` na preparação.
  Nenhum dos dois binários antigos chegou a esse check mais de uma vez, então não há prova de que o motor não mude a
  frequência. Na bisseção (`ec056cf3ff`, só P1-1 e o navegador de mods; binário `f3397b67…`), #44 falhou igual (o status
  depois do save já dizia `Fallout`) e #45 caiu na preparação. A cratera leva sempre **917 ticks** (cerca de 46 s:
  `first_write=88 crater=917` em todas as execuções), e o roteiro detonava a nuke antes de esperar o `/forceload` da área
  da balefire. Sob carga essa espera consumia a cratera. Se o P1-1 alonga essa espera (ele agora enfileira as malhas
  das colunas carregadas durante a espera), a corrida fica mais frequente, mas não há perda: o mundo reaberto segue o
  que foi gravado. Roteiro corrigido em `5b58dfa6`: as duas áreas carregam antes das duas detonações, e um check depois
  do save exige a nuke ainda na cratera (`Planning`/`Writing`/`Waiting`), para que uma execução lenta falhe como
  corrida de preparação, e não como o check do reinício. Com o roteiro corrigido, no binário final: **2/2 inteiros**
  (#50, #52).
- Sonda do `/forceload` no release corrigido: conservou tudo.

**Verificação na cabeça final** (`6a79054fa9`: os três commits acima sobre `7cba121ec`/`621642528`). Release
`horizon-client` `458581ca7d7f7ca00730ac76b2c6dcfcd91a72999e2e5d9b326ac614d46a3072`, pacote igual `9ab18eac…` (o pin
não entra no pacote).

- Motor: `horizon-protocol` `item::hash` 9; `horizon-client-lib` `lan::` 382, `menu::tests` 67, `native_` 26, `mod_` 83,
  `save::` 84, `craft` 41, som 49, `conditional_policy` 4; `horizon-client` `native_` 235, `machines` 27, `mod_` 38,
  `world::` 7, `craft` 32, `lan::` 13, `menu` 36 e os testes novos (eager, `/forceload`, fechamento do criativo, ordem do
  save); `lan_bot` 22; render 72. Clippy `-D warnings` limpo em seis bibliotecas (com `horizon-protocol`), `rustfmt`
  limpo. Os dois testes dos anexos do dono passam (A/B abaixo).
- Em jogo, uma por vez, `NTM_GAME_WIDE=1` (carga 30–42): stations, dims, prova de save `original` `kill` (estrita, igual
  ao controle), bombs (roteiro `5b58dfa6`) passaram. `items` 0/3 inteiros no binário final: duas vezes
  `cancel_does_not_consume` (#46, #53) e uma `thrown_container_waits_its_pickup_delay` (#51), ambos checks de relógio
  real da preparação (o roteiro segura o botão 0,45 s e solta; comer leva 32 ticks, e sob travadas do servidor o
  soltar chega tarde). São as mesmas falhas do `final` `26c81d42` (#1, #10) e do R7c (#9). O `items` inteiro no motor
  novo é o #27 (`1d8d074b`). As provas estritas completas (8/8, 8/8, 3/3) e a sonda
  do `/forceload` foram feitas em `1d8d074b` (`7cba121ec`). Os commits seguintes não tocam o save, o streaming nem o
  `/forceload`: um teste, a concordância do clique LAN e a grade do anfitrião.

**Anexos do dono em saves no meio da sessão** (achado da Onda 1 no pin `a31497f38`): no motor novo já está corrigido
pelo upstream, `61aa03d1a` ("Single-player owner: autosaves keep its live player attachments", 08/11, #12612):
`store_owner` grava os anexos vivos de `guest_attachments`. O teste do upstream
`an_autosave_writes_the_owners_live_attachments_to_the_file_it_loads` e o da Onda 1 trazido para cá
(`the_owner_reopened_after_an_autosave_loads_its_live_attachments`, `10ae4aa43d`) passam no motor corrigido. Num mutante
com o comportamento de `a31497f38` (`store_owner` sem os anexos vivos, worktree descartável), os dois falham
(`lan.rs:4767` e `:4821`; `confirm/attach-ab3.log`).

Lacunas que ficam registradas (sem correção nesta rodada):

- **P3-6** a medição do meltdown não mede o custo do gancho nem prova "não é pior" com força (p ≈ 0,09; 0,22 sem a rodada
  de aquecimento). Falta medir o span `ntm_world_saving` em máquina quieta, com e sem o gancho. Com P2-1 o gancho deixou
  de rodar nos ticks atrasados.
- **P3-7** a quebra por máquina do upstream (#9223, `native_machine_destroy_block`) não resolve multibloco: quebrar um
  dummy deixa o núcleo com buraco, quebrar o núcleo deixa dummies órfãos (sem perda nem duplicação; os dummies não têm
  loot). Nada chama isso hoje; resolver o núcleo como `break_block_as_player` antes de usar.
- **P3-9** com uma passada eager vencida no tick, o settle do NTM roda no meio do tick (antes das entidades): itens de
  uma máquina quebrada entram na passada de entidades desse tick, e não do seguinte. Diferença de um tick, mais perto do
  Java (que derrama na hora); sem perda nem duplicação.
- **P3-10** `frozen-break` sem pareamento de cortes por índice; os três testes de render por GPU de software não foram
  comparados com o `a3bf6531e` puro; o save em etapas (`game/saving.rs`, controle `save` sem servidor integrado) não
  passa por `save_game_with` (preexistente, igual em `a31497f38`; o texto da Onda 0 que diz "todo save passa por
  `save_game_with`" fica corrigido aqui).

### Re-revisão (`cf7685c2`): aprovada com ressalvas

A `ntm-current` do motor foi para `6a79054fa9`. Ressalvas originais, sem bloqueio; o fechamento da retomada está na seção “Fechamento das ressalvas” abaixo:

- **P2-1** a guarda do P1-1 (`no_streaming_pass_drops_the_columns_it_unloads`) é textual e aceita a chamada comentada.
  Falta um teste comportamental: uma passada de `/forceload` que descarrega uma coluna suja e a encontra gravada.
- **P2-2** bombs: mesmo com `5b58dfa6`, a cratera pode acabar entre a leitura do status (depois do save) e o
  fechamento, que grava de novo. Fechar a janela: ler `dug`/status antes do save e fechar logo, ou congelar o tick
  até o fechamento.
- **P2-3** a contagem da sonda do `/forceload` omitia `probe-bug-5` e `probe-bug-6`. Corrigida acima.
- **P3** `items` julga por relógio real (`time.sleep` entre apertar e soltar, `cancel_does_not_consume`,
  `real_eating_started`, `thrown_container_waits_its_pickup_delay`). Converter para ticks do servidor, como o roteiro da
  Onda 1 faz.
- **P3** o teste de `hash.rs` do commit `01e5147ef5` da `rbmk-t02-engine` não passa no `cargo fmt` deste motor (o porte
  `88d6b88a2d` saiu formatado). O RBMK formatou em `00454b508a`; a tabela "Contatos de motor do RBMK" da `rbmk-t03`
  foi atualizada.
- **P3** `host_prepared_craft`: o gancho `prepare_item_craft` roda antes de saber se a retirada cabe. Num shift-click
  sem espaço, ou com o cursor ocupado por outro item, o mod já ouviu (e pode ter feito efeitos colaterais) uma
  retirada que é recusada. O RBMK manteve essa ordem de propósito e a documentou em `host_prepared_craft`: é o contrato
  da grade do convidado (`lan_crafting` pergunta a cada mudança da grade), ou seja, o resultado preparado, não a
  retirada. A glue do NTM não tem efeito colateral ali. No mesmo laço, empilhar no cursor comparava só o item
  (`c.item == result.item`), e um resultado com componentes do mod se juntava a uma pilha sem eles. Corrigido na
  `rbmk-t02-engine` em `00454b508a` (`same_item_same_components`; o mesmo commit formata o teste de `hash.rs`).
  **Falta portar** para a `ntm-current` (`menu_clicks.rs`, perto da l. 784). Fica para a próxima mudança de pin,
  com revisão.

### Merge da `main` (`27380b26`)

Merge sem rebase em `73e5a45e`, sem conflitos (a `main` não toca em `native/`; o pin continua `6a79054fa9`). Depois do
merge: `cargo test --workspace` verde (`ntm-core` 514 e as suítes de integração, entre elas portas, rede de energia e
Watz/PWR), `cargo clippy --workspace --all-targets -- -D warnings` limpo. `cargo fmt --check` acusa 122 arquivos, nenhum
tocado por esta branch: o NTM não segue o rustfmt na `main`. A cópia transformada foi regenerada contra
`6a79054fa9` e o release compilou: `horizon-client` `b84aac6a25fc5c200aa59662a51743d3357edbf058ddf2bd932fc38a01e7a4da`,
pacote de conteúdo novo `ntm.hmod` `483392a4d3640aa4444926f1ca6d44956577d7aefa5143e6764cad94325959a9` (a `main` mudou o
conteúdo). Em jogo, uma por vez, `NTM_GAME_WIDE=1`: stations, dims e a prova de save `original` `kill` passaram (#55–#57).
Clippy `-D warnings` da cópia transformada (`horizon-client-lib`, `horizon-client`, com a glue do NTM) limpo.

### Merge da `main` (`8ecff5d8`)

A `main` andou para `8ecff5d8` (integração T2–T8: gamma, items em ticks, `door_sounds`, bombs). Merge sem rebase em
`ec3e633a`, sem conflitos: `content/` e `native_items.py` ficaram iguais aos da `main` (o roteiro de items em ticks é o
dela). `native_bombs.py` fundiu sozinho: ficou com a leitura da alavanca da `main` e o `restart_arm` de `5b58dfa6`.

- **Geradores como checagem**, na ordem de `16659f63`, no lugar: `generate.py`, `bulk.py`, `generate.py --ids-only` e
  `visual-gen/generate.py`, com `PYTHONHASHSEED=1`, `NTM_SRC` e `HORIZON_RESOURCES`. **Nenhum arquivo mudou**, e as
  quatro portas mantêm o `door_sounds`. (Uma primeira tentativa sem `NTM_SRC` parou no `hazard-gen` antes das abas
  criativas e tirou os títulos das abas das traduções. Os dois arquivos foram restaurados e a rodada refeita.)
- `content-gen`: `test_rbmk_oracle` (contrato T00, só `door_sounds`) passa, e a suíte inteira tem **290 testes OK**
  com `PYTHONHASHSEED` 1 e 42. Os testes leem a fonte em `ROOT.parent/ntm-src`, então foi preciso um link temporário no
  worktree, removido depois.
- `cargo test --workspace` verde (`ntm-core` 514 e as integrações), clippy `--all-targets -D warnings` limpo. A cópia
  transformada foi regenerada contra `6a79054fa9`, e o clippy dela está limpo.
- Release `horizon-client` `b84aac6a25fc5c200aa59662a51743d3357edbf058ddf2bd932fc38a01e7a4da` (igual ao do merge
  anterior: a glue não mudou). Pacote `ntm.hmod` `483392a4d3640aa4444926f1ca6d44956577d7aefa5143e6764cad94325959a9`,
  empacotado do conteúdo regenerado sem diferença. É igual ao anterior: entre `27380b26` e `8ecff5d8`, em `content/`,
  só `ids.json` mudou, e ele não entra no pacote.
- Em jogo, uma por vez, `NTM_GAME_WIDE=1`: stations, dims e a prova de save `original` `kill` passaram (#60–#62).

### Pin `eb5d3eed58`: contatos de motor da RBMK T03 (10/11)

A branch de atualização `upstream-a3bf653-fix` avançou por fast-forward de `6a79054fa9` para `643e359566` (a ponta da
`rbmk-t03-engine`):

- `56fbb210a7`: colunas tão altas quanto cada uma foi posta (= `5b9d6ee175`).
- `3b96e2d998`: o `column_core` acha o núcleo descendo pela própria coluna (= `06bf7ffe5c`).
- `643e359566`: o resultado do craft só se junta ao cursor com os mesmos componentes (= `00454b508a`, portado à mão).

Por cima entra `eb5d3eed58` (P3-3 da revisão do merge do T03): o slot 302 da fornalha (`FurnaceResultSlot`) também só
se junta ao cursor com `same_item_same_components`. Nenhuma outra fusão no cursor compara só o item
(`pad_craft.rs:97` compara para reconhecer a receita). Naquela rodada, `643e` e `eb5d` ficaram sem teste
comportamental. A retomada extrai o caminho de retirada do craft e cobre componentes, recusas e confirmação; o
clique específico de resultado da fornalha (`eb5d`) ainda fica como lacuna nominal.

Pin do NTM movido em `ab5bf359` (branch `ntm-pin-rbmk-t03`, a partir da `main` `e4aa8b2c`; nada fundido na `main`).
Verificação (`confirm/pin-verify.log`):

- Testes: `horizon-world` `mod_multiblocks` 7; `item::hash` 9; client-lib `menu::tests` 67, `lan::` 382, `craft` 41,
  `native_` 26, `mod_` 83, `save::` 84; client `craft` 32, `menu` 36, `lan::` 13, `native_` 235, `machines` 27,
  `mod_` 38; `lan_bot` 22.
- Clippy `-D warnings` limpo em seis bibliotecas, `rustfmt` limpo.
- A cópia transformada foi regenerada contra `eb5d3eed58`, e o clippy dela está limpo.
- Release `horizon-client` `d3ae3ad109083249c0a722ae15d9cdb8bc1003d3bf970a6770ec2d6baaeac038`. Pacote `ntm.hmod`
  `483392a4d3640aa4444926f1ca6d44956577d7aefa5143e6764cad94325959a9` (o conteúdo não mudou).
- Em jogo, uma por vez, `NTM_GAME_WIDE=1`: stations, dims e a prova de save `original` `kill` passaram (#64–#66).

Depois a `main` foi para `71f92459` (Onda 1), e ela entrou nesta branch por merge sem rebase em `ce5a29ca`, sem
conflitos. Os geradores rodados como checagem (ordem de `16659f63`) não mudaram nenhum arquivo; `test_hazmat` 9 e
`test_rbmk_oracle` 14 passam. O clippy da cópia transformada recusou `HeldKeys::contains` no tooltip dos mods de
armadura que a Onda 1 trouxe (`armor_table.rs:574`, `872dfd7f`, escrito no motor antigo). Trocado por `raw` em
`55198727`: é o mesmo teste (`contains` só repassa) e o do Java (a tecla LSHIFT literal). A própria `main` `71f92459`
falha nesse clippy sem essa troca. Release `horizon-client` `5db4d5f48527e176bc0215b89f6d33d148bff3350b8f4fc5f37aa56dac9205ea`,
pacote `ntm.hmod` `8aad9aaa6307cd044e86f7c90d8d52f7f8814d61c6eabc613bd0e01174597a7b` (conteúdo da Onda 1). Clippy da
cópia transformada limpo; em jogo, stations, dims e a prova `kill` (#68–#70) e stations de novo no binário final (#72).

### Limitações e pendências

- **Modo eager:** os cortes dentro de um save (`save_begin`, `live_chunk`, `region_record`, `entities_written`) não são
  comparáveis com a gravação eager ligada (o controle grava o chunk antes, o `live_chunk` nunca dispara nele, o índice
  de registro cai em outros chunks). Ficam provados no modo estrito.
- O roteiro de `frozen-break` não pareia cortes por índice como o `original`; `region_record:1`, acrescentado nesta
  rodada, ficou sem par (NTM 12/12, controle 2/12).
- O teste de jogo com mod contador (`7cba121ec`) distingue o gancho eager; a antiga afirmação de que só havia teste
  de estrutura estava desatualizada. A limitação do modo eager para comparar cortes de crash permanece acima.
- `audio-lifecycle` em máquina quieta, com os dois binários.
- O pipeline dos modelos OBJ não recebe a deformação `world_warp` nem as marcas de luz HDR das entidades.
- Cinco testes do render falham neste ambiente sem diff local: dois de pilha de crash (sem debuginfo, desligado para
  caber no disco) e três de GPU por software; não comparados com o upstream puro.
- O teardown do renderer GL/llvmpipe entrou em pânico uma vez (`SurfaceId`) ao fechar o `items`; não se repetiu.
- Upstream continuou andando depois de `a3bf6531e`; o próximo snapshot fica para a próxima rodada.
- Achado do RBMK T03 (10/11, pin `a31497f38`, não verificado nem corrigido nesta rodada): visitar um convidado LAN a cada
  tick com `native_visit_player` (`enter_guest`/`leave_guest`) enquanto ele queima e entra na água traz o fogo de volta
  (`Fire` 600 nas leituras seguintes; -20 sem a visita). Suspeita: a ida e volta da troca de estado regrava um
  `fire_ticks` antigo por cima do da cópia de movimento do convidado (`MotionWorld`, `lan_actions.rs`), onde a água
  apaga. Vale para qualquer mod que visite por tick. O NTM contorna em `rbmk-t03` `52d7bb62` (não visita na água).
  Continua sem diagnóstico: caso a verificar.
- Outros contatos de motor do RBMK T03 (revisão do T03, não reproduzidos em jogo, sem correção nesta rodada):
  - Em `motion/environment.rs` (l. 37–45), `tick_remaining_fire` aplica o dano de fogo antes de a água apagar. Um fogo
    de 600 no tick em que se entra na água custa 1 de dano (600 % 20 == 0). No 1.7.10, `handleWaterMovement` zera o
    fogo antes do dano do `onEntityUpdate`.
  - O atributo `burning_time` soma a Proteção contra Fogo das peças. O 1.7.10 usa o maior nível da armadura
    (`EnchantmentProtection.getFireTimeForEntity`). O HOT do NTM calcula como no 1.7.10 (`set_remaining_fire_ticks`),
    mas os outros fogos do motor seguem somando. Lacuna nominal da Onda 4 (regra de armadura).
  - Barra de durabilidade: o Forge 1.7.10 desenha preto 13×2, uma faixa 12×1 na cor `(255−k)/4 << 16 | 0x3F00` e a
    barra. O gancho `item_bar` (API 60) só dá largura e cor, então a faixa escura não sai. Falta um campo de cor da
    faixa na API.

### Fechamento das ressalvas (retomada da revisão)

Trabalho isolado em branches de revisão (sem fast-forward de `main` nem de `ntm-current`):

- **Motor (`horizon-review2` sobre `eb5d3eed58`):**
  - `87b9f4ddb9`: `Hashed::matches` em `lan/containers.rs` (`hashed_matches`) passa a emprestar `&self.sync` sem clonar (**P3-4**); `save.rs` ganha o teste `eager_policy_tests::eager_saving_respects_the_switch_and_an_armed_failpoint` (**P3-2** / `dce43d298`).
  - `2187c3192b`:
    - **P2-1 e `ec056cf3f`:** `no_streaming_pass_drops_the_columns_it_unloads` e `every_creative_close_lets_go_of_the_mod_browser` (`native_hooks_tests.rs`) removem comentários `//` antes da inspeção; o teste comportamental `forceload_wait_saves_and_announces_the_dirty_column_it_unloads` (`game_tests.rs`) aciona `wait_for_forced_chunks` com chunk sujo e verifica gravação via `save_later` e anúncio único aos mods (`chunks_changed`).
    - **P3-2 (`7c6f44d08`):** `declared_mod_menu_slots_override_the_block_entity_storage` (`game_tests.rs`) prova que `slots()`/`slot_count()` respeitam os slots declarados do menu acima do tamanho do block entity.
    - **P3 (`host_prepared_craft`):** `take_host_craft_result` / `take_host_craft` (`menu_clicks.rs` e `inventory_slots.rs`) checa destino (cursor compatível com `same_item_same_components` ou espaço no inventário) antes do gancho, valida item/quantidade/componentes de forma atômica com `"replace_existing": true` para `prepare_item_craft` em `native-hooks.json` (`horizon-mod-api-gen`, permitindo substituir `Some` ou `None` sem manter o resultado anterior), só dispara `native_item_crafted` após inserção confirmada e documenta o contrato puro em `docs/native-mods.md`. Coberto por `host_craft_checks_the_destination_before_preparing_and_never_inserts_partially`, `craft_dispatch_preserves_existing_and_missing_results_without_a_replacement` e `craft_dispatch_replaces_some_or_none_and_commits_only_after_inserting`.
- **NTM (`ntm-horizon-review2` sobre `main` `318887c9`):**
  - `59dd06fc`: `tools/test-game/native_bombs.py` (`restart_arm`) congela os ticks (`tick freeze`) antes de ler `status` e `dug`, grava `bombs-restart.json` e mantém os ticks congelados até fechar o jogo, com desbloqueio em `restart_check` (**P2-2**), testado por `tools/test-game/test_native_bombs_restart.py` (`RestartOrder`); `tools/test-game/native_items.py` lê `active.get('using')` como `dict` (`{'slot', 'ticks'}`) ou lista e exige `0 <= elapsed < 32` em `real_eating_started` (**P3**).
  - `3dab1435`: em `tools/test-game/native_items.py`, `fallout` volta ao criativo antes da espera de 15 s com `random_tick_speed 1000` (evitando morte por radiação acumulada antes do `chunk_field`) e `reset()` faz `respawn` se o jogador estiver morto.
- **Verificação:**
  - **Mutantes e motor (`horizon-review2` @ `2187c3192b`):** `mutants.py` (`MUTANTS-PASS`: linha de base e restaurações verdes; mutantes de `retire_unloaded` removido, comentado com `//` e comentado com `/* ... */` mortos comportamentalmente por `forceload_wait_saves_and_announces_the_dirty_column_it_unloads`, e removido/`//` também por `no_streaming_pass`; mutante `replace_existing: false` morto por `craft_dispatch_replaces_some_or_none_and_commits_only_after_inserting`); testes do motor (`horizon-mod-api-gen` 18, `horizon-mod-api-gen-tests` 13, `horizon-protocol item::hash` 9, `horizon-client-lib` `eager_saving_respects` 1 e `lan::` 382, `horizon-client` `menu` 36, `craft` 32 e `--include-ignored` `declared_mod_menu_slots` 1, `host_craft_checks` 1, `no_streaming_pass` 1, `every_creative_close` 1, `an_eager_pass_is_heard` 2); clippy `--locked ... -D warnings` e `cargo fmt --all -- --check` limpos.
  - **NTM e cópia transformada (`ntm-horizon-review2`):** `cargo test --workspace --locked --lib --bins` (728) + 12 suítes de integração em `crates/ntm-core/tests/*.rs` exceto `save_model` (121 testes), `cargo clippy --workspace --all-targets --locked -- -D warnings` limpo; suíte unitária Python em `tools/test-game/test_*.py` (10/10 OK após `3dab1435`). Clippy da cópia transformada em `stage` (`horizon-client-lib`, `horizon-client --lib -D warnings`) limpo.
  - **Binários finais:** `horizon-client` `40165d696c189779b6dae376834d54b7a9cd8c93668c071e5019cf8093aa9fea` e pacote `ntm.hmod` `eebe930a4a39dacf367092946374fbeb2f536d9df732bda37bdc6216051bc23f` (compilados de `horizon-review2` `2187c3192b` sobre a árvore NTM/conteúdo de `main` `318887c9`; todos os commits de `ntm-horizon-review2` alteram só scripts Python de teste e documentação).
  - **Em jogo (`NTM_GAME_WIDE=1`):** `bombs` **2/2** (`ok: true`, congelado em `Planning R=80` no `bombs-restart.json` e `wnativen4: a nuke's crater stops with the world` na reabertura), `items` completo com `NTM_TEST_HOOKS=1` (**329/329** checks: `setup` 309, `capture-state` 12, `check-state` 8) e prova estrita de save `kill` com `HORIZON_NO_EAGER_SAVE=1` (`ok: true`, critério `last_save`, `proof: true`, `no_duplicate: true`, `no_duplicate_item: true`, `lost: 0`, `control_lost: 0`, `storage_clean: true`).
- **Lacunas nominais remanescentes:** clique específico do resultado da fornalha (`eb5d3eed58`), formatação da branch antiga `rbmk-t02-engine` (`01e5147ef5`) se ainda vier a ser fundida, e os itens preexistentes de `Limitações e pendências` acima (incluindo o caso de `fire_ticks` do RBMK T03).

## Motor: anexos do dono num save no meio da sessão (Onda 1, 10/11/2026)

Defeito: enquanto o dono do mundo (um jogador) joga, os anexos dele (`ntm:item_state`, `ntm:armor_table`) ficam em
`Game::guest_attachments`; `store_owner` gravava a cópia vazia do registro do convidado em
`players/data/<uuid do singleplayer>.dat`, o arquivo de onde o dono carrega. Um crash depois de um autosave perdia
RadAway, enjoo herbal, cooldowns e as peças da mesa de armadura; a dose só sobrevivia pelo espelho antigo no KV
(`docs/onda-1-protecao.md`). O upstream corrigiu em `61aa03d1a` (#12612).

Branch do motor `player-attachments-fix` sobre o pin `a31497f38`: só `git cherry-pick 61aa03d1a`, commit
`130153a4ac` (refeito depois da queda da máquina de 10/11 07:14, que deixou vazio o primeiro objeto do
commit; conteúdo igual, conferido contra o upstream). A correção em
`store_owner` entrou sem conflito. O conflito foi só no teste: o upstream o põe depois de um módulo
`world_spawn_sync_tests` que o pin não tem, e usa `native_machines::tests::server_game` e
`world_spawn_sync_tests::host_for`, que também não existem aqui (nem `NetConfig::loopback`, e o `Joined` do pin não tem
`expected`/`configured`). Adaptação mínima: o módulo `world_spawn_sync_tests` não entra; o teste do upstream entra igual,
com cópias locais dos dois ajudantes, cortadas ao que ele usa (sem o convidado no Nether; `NetConfig::lan` com
`bind` em localhost no lugar de `loopback`).

| Ponto | Arquivo | Função | Linha (branch) |
| --- | --- | --- | --- |
| Correção | `crates/horizon-client/src/game/lan.rs` | `store_owner`: troca os anexos do registro pelos vivos só durante `playerdata::store` e devolve | 1634, troca em 1653-1663 |
| Referência (já certo) | `crates/horizon-client/src/game/lan.rs` | `save_guest`: quem sai grava os anexos vivos | 1423 |
| Única chamada | `crates/horizon-client/src/game.rs` | `save_game_metadata` → `store_owner` | 15375 |
| Autosave | `crates/horizon-client/src/game.rs` | `save_game_with(false)` no contador de autosave | 7542 |
| Save da pausa | `crates/horizon-client/src/game/integrated.rs` | `save_game_with(false)` no tick que pausa | 661 |
| `/save-all` | `crates/horizon-client/src/game/commands/profiles.rs` | `cmd_save_all` → `save_game` | 56 |
| `horizon-ctl save` | `crates/horizon-client/src/game/control.rs` | `control_save` → `save_game` | 1087 |
| Save de saída | `crates/horizon-client/src/game/saving.rs` | `poll_closing_save` → `save_game_metadata` | 192 |
| Leitura | `crates/horizon-client/src/lan/playerdata.rs` | `load_owner` lê o arquivo do dono | 84 |
| Teste do upstream | `crates/horizon-client/src/game/lan.rs` | `owner_attachment_save_tests::an_autosave_writes_the_owners_live_attachments_to_the_file_it_loads` (`#[ignore]`, precisa de `HORIZON_TEST_RESOURCES`) | 3180 |
| Teste extra (`b22f1fbef1`) | `crates/horizon-client/src/game/lan.rs` | `owner_attachment_save_tests::the_owner_reopened_after_an_autosave_loads_its_live_attachments` | 3216 |
| Ajudantes adaptados | `crates/horizon-client/src/game/lan.rs` | `server_game`, `host_for` (cópias locais) | 3055, 3093 |

Todos os caminhos de save do dono passam pela mesma `store_owner`; o teste do upstream a exercita direto, mas confere
só o `level.player_extra` em memória. Por isso há um segundo commit na branch, `b22f1fbef1`, com um teste meu no mesmo
módulo (`the_owner_reopened_after_an_autosave_loads_its_live_attachments`): `store_owner` → `Level::write` →
`Level::read` → `load_owner`, o dono reabrindo depois de um crash. Os dois passam (`--ignored`, com
`HORIZON_TEST_RESOURCES`); com a correção revertida (mutação) os dois falham (`None` em vez de `Some(7)`); clippy
`-p horizon-client --lib --tests -D warnings` limpo. Hash e teste enviados ao agente do Horizon (1c5fa695). A saída e
volta ao mundo também ficam provadas em jogo (cortes de save da Onda 1, `docs/onda-1-protecao.md`).

**Depois (10/11):** a `main` `e4aa8b2c` passou ao pin `6a79054fa9` (upstream `a3bf6531e`), que já contém
`61aa03d1a` com o `store_owner` idêntico byte a byte ao de `130153a4ac` e o teste extra como `10ae4aa43d`
(confirmado pelo agente do Horizon). A branch `player-attachments-fix` não entra no pin; ela fica como prova
da correção sobre `a31497f38`.
## Pedido ao motor em aberto: listas de slots por célula de multibloco

Hoje a política de slot do contêiner nativo é uma por máquina e não conhece a célula pela qual um funil chega. A soldadora a arco
do Java (`getAccessibleSlotsFromSide(x,y,z,side)`) dá uma lista diferente a cada célula (entradas 0, 1 ou 2 e a saída; nada nas seis
células de cima). Detalhe e o que o port faz no lugar em `docs/native-slot-policy.md` (seção "Pedido ao motor: lista de slots por
célula") e em `docs/onda-2-sublote-1.md`.

