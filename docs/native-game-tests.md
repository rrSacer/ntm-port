# Roteiros de aceitação no jogo nativo

Os roteiros novos usam o jogo com NTM e Recipes de `tools/build-native.sh`,
num mundo criativo de teste. Todos os jogos e builds passam pelo `agent-run`,
em Xvfb mudo e com homes descartáveis. Nunca use um mundo real.

## Prensas

Com o jogo aberto e `control.sock` na pasta de teste:

```sh
python3 tools/test-game/native_presses.py /caminho/do/jogo prensa eprensa
```

O driver usa o socket diretamente, grava cada comando e resposta em
`native-presses.json` e sai com erro se uma conta falhar. `native_game.py`
implementa a espera por jogador/chunks, inserção por cliques reais e a posição
absoluta de botões de `horizon_screen`; não altera estado interno de máquina.
Para multiblocos, a prensa é aberta pelo dummy superior visível.

| Fase | Conta preservada de `presses.py` |
| --- | --- |
| prensa | 8 lingotes → 8 chapas; matriz com desgaste 8; 8 chapas retiradas ao inventário |
| eprensa | matriz de fio com desgaste 62 + 5 lingotes de cobre → 16 fios; sobram 3 lingotes e a matriz quebra |

A elétrica recebe uma bateria criativa no slot 0. Isso isola o processamento
da aceitação das redes HE, a cargo de `native_redes.py`.

### Prova de 30/10/2026

Motor `9803b72cd`, bundle NTM `db859b71`, Recipes oficial `ae217daa`, conteúdo
fixado da L2 (`ntm-l2c.hmod`). Binário fast isolado da prova V5; o novo build
release de `build-native.sh` foi interrompido antes de terminar, portanto esta
rodada não comprova esse build release.

- **Prensa passou:** 8 chapas no inventário, desgaste 8 da matriz.
- **Elétrica falhou, pendência enviada ao responsável pela lógica:** os três
  itens entram nos slots, mas a bateria criativa não carrega a máquina.
  Após 180 segundos: energia 0/50.000 HE, matriz ainda com desgaste 62,
  cinco lingotes e nenhum fio. O roteiro preserva o erro, sem afrouxar a conta.
- **Centrífuga a gás:** a entrada nativa será testada por barril e dutos UF6.
  O botão antigo da tela WASM não representa um slot nativo. O responsável
  confirmou que o slot 5 é identificador, o 6 é upgrade e o tanque inicia UF6.
  Mudar o modo do barril exige a correção de botões `7b56d7d7` ou posterior.

Evidência local: `.agent-tmp/c65d-native-tests/runs/game-7gx97fwq/`, no diretório
pai dos repositórios. `native-presses.json` conserva os comandos e checks;
`native-presses-prensa.png` mostra a saída e o desgaste;
`native-presses-eprensa-stalled.png` mostra os slots parados. O compound da
elétrica está também em `.agent-tmp/c65d-native-tests/epress-data.json`.

## Continuação

Fundição, bigorna, triturador, centrífuga, bedrock, petróleo A–D, purex e m3
continuam em adaptação. Falhas de lógica são encaminhadas ao responsável por
`ntm-core`; os roteiros não corrigem `crates/` nem injetam estado para passar.

## Fundição nativa

`foundry.py` aceita `HORIZON_CTL` (o ctl do build), prepara chunks e chão antes
do teleporte e preserva as fases `fundir`, `canais`, `reter`, `voltar`.
O setup limpa do inventário somente o item de entrada antes de dar o lote:
sem isso o shift-click realimentava a máquina com lingotes recuperados na
fase anterior, invalidando a conta de entrada.

Prova em Xvfb mudo, server-thread, com o mesmo fast/V5 e bundle fixados acima:

| Fase | Resultado |
| --- | --- |
| fundir | 3 lingotes no cadinho → 3 retirados; inventário 3; ~65 s |
| canais | 2 lingotes novos → 2 retirados após canais/tanque/saída; ~25 s |
| reter | Molde retirado, 3 lingotes novos, tanque com metal e inventário 0; salvo |
| voltar | Mundo copiado para outra pasta descartável e reaberto; molde recolocado → 3 lingotes recuperados, inventário 3; ~43 s |

As contas de saída são assertivas: qualquer quantidade diferente da esperada
encerra o roteiro com erro. `py_compile` e `git diff --check` também passaram.
Nenhum `took too long` nos logs.

Evidência da sequência exata: `.agent-tmp/c65d-native-tests/runs/game-wn3b0ua3/`;
reabertura em `.agent-tmp/c65d-native-tests/runs/game-vknf39zk/`.
Cada pasta contém `foundry-evidence.json`, `client.log` e capturas.

## Lista de receitas (L2)

`native_recipes.py` (fases `montadora` e `bigorna`) abre a lista pelo botão de receita da tela da máquina, busca, escolhe, fecha e confere que a máquina usa a receita. As teclas da montadora são eventos X11 reais (`xinput.py`, XTest; é o caminho do `WindowEvent::KeyboardInput` do jogo); a bigorna usa `horizon-ctl key` e `type` (o caminho do ctl), com texto acentuado.

```sh
python3 tools/test-game/native_recipes.py /caminho/do/jogo montadora bigorna
```

| Fase | Conta |
| --- | --- |
| montadora | Enter foca a busca; `ferro` filtra; o primeiro resultado é a Placa de Ferro; antes de fechar o servidor não tem a escolha (data 0); Escape volta à máquina e o data slot 30 vale 1 (receita 0 + 1); 4 lingotes de ferro e uma bateria alimentada pela rede fazem 4 chapas de ferro |
| bigorna | bigorna de desh; `aço` filtra; o primeiro resultado é a Placa de Aço; o botão Fechar manda a escolha (data 1 = 5); 20 lingotes de aço, "Construir uma vez" faz 1 chapa e Shift faz as outras 19 |

Prova de 30/10/2026: motor `b9d905189` (API 31), bundle NTM com a lista, Recipes oficial `ae217daa`, conteúdo do HEAD do port (`56d6cff4`, com a textura do seletor). Capturas e o JSON dos comandos ficam na pasta de cada jogo (`native-recipes*.png`, `native-recipes.json`).

## Máquinas com lista de receitas (L2) e as redes

`native_stations.py` (uma fase por máquina) escolhe a receita na lista nativa (`native_recipes.pick`, com `horizon-ctl key` e `type`), alimenta a máquina e confere o que sai. A energia vem de uma bateria em buffer ao lado da máquina, enchida por uma bateria criativa num funil (ou do slot de bateria, quando a máquina carrega por ele); os fluidos vêm de um barril com recipientes cheios, no modo saída, e de um duto tipado com o identificador de fluido.

```sh
python3 tools/test-game/native_stations.py /caminho/do/jogo planta misturador purex m3_fluidos m3_centrifuga
```

| Fase | Conta | Resultado em 30/10 |
| --- | --- | --- |
| planta | Dinamite: açúcar, salitre e areia dão 2 dinamites | passou |
| misturador | Refrigerante: 2 baldes de água num barril, duto de água, salitre; 2000 mB de refrigerante (sobram 200 de água), com a bateria criativa no slot dele | passou |
| purex | Haste de Plutônio-239: ácido sulfúrico por barril e duto; 2 tarugos de Pu-239 e 1 de urânio | passou |
| m3_fluidos | Oxigênio: 8 tanques de ar num barril, duto de ar, planta química; 500 mB de oxigênio | passou |
| m3_centrifuga | 2 células de UF6 num barril, duto de UF6, centrífuga a gás com a bateria criativa no slot 4; uma pepita de U-238 | passou (30/10), depois do `b5abf180` |

Também passaram em jogo: `native_presses.py eprensa` (16 fios, 3 lingotes, matriz quebrada), `native_anvil.py` (forja: bigorna de ferro + 12 de aço dá a de aço) e `native_recipes.py` (montadora e bigorna).

Evidência: `.agent-tmp/sonnet-l2/runs/` (cada pasta tem `native-stations.json`, `client.log`, capturas). Motor `b9d905189`, bundle NTM e Recipes oficial `ae217daa`.

## Cliente da radiação e relayout (L4)

`native_client.py` (fases `hud`, `tooltip`, `eletrolisador`):

| Fase | Conta |
| --- | --- |
| hud | A caixa do Geiger aparece no canto superior esquerdo (pixels de uma das quatro cores da dose), antes e depois de 64 tarugos de urânio na mão; há captura de cada |
| tooltip | O ponteiro X11 real sobre uma pilha de 64 tarugos de urânio: `[Radioactive]`, `0.175RAD/s`, `Stack: 11.2RAD/s` |
| eletrolisador | A tela de fluidos mostra os slots 3 a 13 e deixa 14 a 20 fora; o botão de eletrólise de metal mostra 14 a 20 e esconde 3 a 13 na hora; o botão de volta restaura a primeira |

Prova de 30/10: motor `b9d905189`. A fase `hud` só passa num jogo cujo servidor grave `ntm_rad_dose` e `ntm_rad_rate` por jogador; a prova abaixo usou, **só no build de teste**, uma sobreposição que grava 321,5 RAD e 1,819 RAD/s no hook de tick do servidor (nunca commitada), por o servidor ainda não publicar esses valores. As capturas mostram `NTM Geiger | 1.819 RAD/s`, `Dose: 321.50 RAD` em amarelo (200 a 400 RAD) e a dica do tarugo de urânio.
