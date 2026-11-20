# O arquivo (`ntm:filing_cabinet`)

Fonte: `TileEntityFileCabinet.java`, `ContainerFileCabinet.java`, `GUIFileCabinet.java`, `BlockDecoContainer.java` e
`TileEntityCrateBase.java` (a base de que ele herda). Antes era um bloco inerte do catálogo K3 (`docs/content-catalogo-k3.md`);
agora guarda itens.

## O que faz

- **Oito lugares**, em duas fileiras de quatro (`super(8)`; slot `j + i * 4` em `(53 + 18j, 18 + 36i)`). A tela é a do
  `gui_file_cabinet.png` (176x170): título ao centro, inventário do jogador abaixo.
- **Só uma mão mexe nele.** `isItemValidForSlot`, `canInsertItem` e `canExtractItem` são falsos para todos os lugares ("No automation,
  it's a filing cabinet"): um funil por cima não põe nada, um funil por baixo não tira nada. Nenhuma face fica aberta.
- **Shift-click** (`transferStackInSlot`): de um lugar do arquivo para o inventário (inclusive a hotbar, de trás para frente),
  e do inventário para o primeiro lugar livre do arquivo (`mergeItemStack(.., 0, 8, false)`).
- **Quebrado**, devolve os oito lugares (`BlockDecoContainer.breakBlock`): cada pilha, uma vez, com os dados do item. É o motor que
  derrama os lugares; o modelo do mod não guarda nem larga item nenhum (`Logic::contents` vazio), para não haver duas cópias.
- O item fica **inteiro**: o registro do modelo não guarda os lugares (`slots_recorded` falso, `#[serde(skip)]`), então nome,
  encantamento e qualquer componente que o mod não lê continuam como o motor os gravou; uma restauração do diário toma os lugares do
  bloco do motor.
- Mesmos estados do bloco K3 que já existia (`facing`, `honey_level`), mesma textura de aço, mesma dureza.

## O que fica de fora (declarado)

| Lacuna | Onde está na fonte | Por que espera |
| --- | --- | --- |
| **A trava** (cadeado, chave, `key_kit`): o arquivo herda de `TileEntityLockableBase`; um cadeado no bloco bloqueia a tela e o funil, e `onBlockActivated` deixa o cadeado passar para o bloco | `TileEntityCrateBase.canInsertItem`/`canExtractItem` (`!isLocked()`), `BlockDecoContainer.onBlockActivated` | Reconectar quando a fatia da trava existir: hoje nenhuma máquina do mod expõe `isLocked`/`canAccess`. Aqui o arquivo está sempre destrancado. |
| Gavetas que abrem (`lowerExtent`, `upperExtent`, o modelo do `RenderFileCabinet`) | `updateEntity`, `getRenderBoundingBox` | O bloco é o cubo estático do K3 (textura de frente e lateral de aço); sem animação, sem rede de pacotes do arquivo (`networkPackNT`). |
| Sons `block.crateOpen` e `block.crateClose` | `openInventory`/`closeInventory` e `updateEntity` | Dependem da contagem de jogadores usando e das gavetas; ficam com a animação. |
| Caixa de colisão fina (`setBlockBoundsTo(.1875, 0, 0, .8125, 1, .75)`) | `ModBlocks.java:1594` | O K3 usa o cubo inteiro. |
| Nome do arquivo trocado na bigorna (`customName`) e as aranhas (`hasSpiders`, `fillWithSpiders`) | `TileEntityCrateBase` | Sem origem no jogo: nenhum item nem gerador do mod os usa. |

## Como foi provado

- **Contas do modelo e das regras do motor** (`cabinet.rs`, `native.rs`, `furnaces_tests.rs`): oito lugares; só a mão passa pelas regras
  de inserir e extrair, em todos os lugares e nos dois sentidos; nenhuma face aberta; o registro não guarda os lugares; pela ponte, o
  que a mão põe fica exatamente como está (item com dados incluído) durante 40 ticks, e um lugar esvaziado fica vazio. Mais um teste
  geral, `every_slotted_block_builds_a_slotted_machine`: um bloco da tabela `SLOTTED` cuja entrada do registro ainda construísse uma
  máquina inerte teria os lugares do motor apagados pela ponte (foi o que uma entrada K3 antiga fez na primeira versão deste
  trabalho: o teste do arquivo pegou, e agora a classe inteira é coberta).
- **Mutantes** (11, todos mortos): funil passa, nono lugar, nove lugares, o registro guardando os lugares, uma face aberta, o funil
  levando o lugar 0, o registro dizendo inerte, a regra sem despacho, o contêiner da mesa do chaveiro, fora da tabela da ponte, não
  `Slotted`.
- **No jogo** (`tools/test-game/native_cabinet.py`, cenário `cabinet` do `run-native-reactors.py`): a mão enche os lugares (uma espada com
  nome e encantamento entre eles, que sai igual); shift-click para dentro (primeiro lugar livre) e para fora (a pilha inteira); funil
  por cima e por baixo não mexem; quebrado, cada pilha cai uma vez; o mundo gravado com os oito lugares cheios, reaberto, mostra o mesmo.
