# Identificador múltiplo de fluidos

O catálogo inclui `ntm:fluid_identifier_multi`, stack máximo 1, com os nomes originais em português e inglês e a tag `ntm:fluid_identifiers`. O fonte atual registra apenas `ItemFluidIDMulti` (`ModItems.java:3864`); `hbm:item.fluid_identifier` consta em `MainRegistry.ignoreMappings`. Por isso não foram criadas variantes artificiais do identificador antigo.

Contrato combinado com a lógica nativa: `minecraft:custom_data` contém `{ntm_fluid:"water"}` (ou outro `id` de `content/fluids.json`). Sem tipo, o padrão é `none`. Exemplo:

```mcfunction
/give @s ntm:fluid_identifier_multi[minecraft:custom_data={ntm_fluid:"water"}] 1
```

No original, `fluid1` e `fluid2` guardam IDs numéricos no NBT, o metadata acompanha o primário, e `IItemFluidIdentifier.getType` retorna o primário. `FluidDuctBase.handleActivation` lê essa interface para tipar o duto. O clique normal troca os dois tipos; Shift abre a GUI; ALT no duto captura seu tipo. Esses controles, o segundo tipo e a seleção pela GUI ficam para os hooks nativos. O conteúdo declara o contrato do primário sem simular essas ações.

A base visual é `fluid_identifier_multi.png`, com `fluid_identifier_overlay.png` no segundo layer. O original multiplica o overlay pela cor do fluido primário. O modelo estático usa a cor `888888` de `NONE`, extraída pelo mesmo parser que gera `fluids.json`. **Tint dinâmico por custom_data é uma lacuna da camada nativa.**

A receita original (`CraftingManager.java:853`) é vertical: um corante genérico, um circuito de válvula e uma placa de ferro. O wildcard de saída serve ao NEI no fonte e é normalizado para o único item, cujo tipo é dado pelo NBT. O OreDict `dye` inclui as 16 identidades vanilla da versão original e os registros explícitos do NTM, inclusive famílias wildcard. Isso também libera a receita original de `template_folder` (linha 585). São 938 receitas vanilla e 2.701 IDs; a aba original de moldes passa a aparecer no criativo.

Solução reaproveitada: extensão de `catalog_completion`, tradutor de receitas `bulk_recipes`, overlay PNG já existente, tag pelo gerador e perfil adicional no verificador de conteúdo em jogo. Não há novo empacotador nem lógica de uso de item neste lote.

Validação: 170 testes Python em cada seed (`PYTHONHASHSEED=1` e `42`), `validate-bulk.py` e `tools/recipe-gen/validate.py` passaram. A prova em Xvfb no ntm-next `c6ad4a73e` com bundle NTM/Recipes passou em 25 verificações: item recebido, componente `ntm_fluid:"water"` presente no inventário e no NBT retornado por `data get entity`, tag reconhecida (`Found 1 matching item(s)`), ícone renderizado e nenhuma rejeição de conteúdo ou panic. Isso prova o contrato de conteúdo; a tipagem funcional de duto/barril é trabalho da lógica nativa.

Evidências locais: `content-gen-evidence/fluid-identifier/` (logs, `game.json`, captura `items.png`). Pacote persistente: `dist/ntm-content-fluid-identifier.hmod`, 6.527.413 bytes, SHA-256 `630b80dd6bd6fb4c4ada3a3dfa2d6b04ea3ed6d4d517e6aafc55a7c6702346e9`.

O `cargo-horizon-mod check` do fork atual e o do upstream passaram no mesmo pacote acima, ambos como conteúdo puro. A pasta de mundo descartável foi removida após guardar a evidência; instalações e mundos do usuário não foram alterados.
