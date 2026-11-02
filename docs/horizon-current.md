# Horizon atual e bombas otimizadas

Decisão do usuário em 01/11/2026: trabalhar com o Horizon oficial mais recente,
adaptando o NTM quando as interfaces mudarem. O visual aprimorado foi arquivado;
suas configurações antigas são migradas para o otimizado sem apagar as demais opções.

## Base de desenvolvimento

- Projeto: `/mnt/ssd120/sacer/horizon-ntm`.
- Motor: `engine/horizon-ntm-next`, branch `ntm-current`.
- Upstream incorporado: `5d65d7b50dbf1b98d43d201bd83e008edd55369e`.
- Pins do motor, mixins e Recipes: `native/ntm/compatibility.json`.
- A antiga branch `ntm-next` e os trabalhos pendentes de catálogo e energia
  continuam preservados. Não usar esses pins antigos para novos desenvolvimentos.

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

Os executáveis são escritos em `$CARGO_TARGET_DIR/release`. `NATIVE_PROFILE=fast`
permite uma compilação de teste. `SKIP_UPSTREAM_CHECK=1` só serve para repetir ou
reproduzir uma revisão já selecionada; sem essa opção, toda execução consulta o
Horizon oficial. O script não instala o jogo e não abre mundos do jogador.

Validação desta atualização será registrada no artifact `horizon-atualizacao`.
