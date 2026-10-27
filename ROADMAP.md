# **Forge Roadmap**

Cada item abaixo corresponde a uma issue do [Forge Backlog](https://github.com/users/RGomes98/projects/1) e a cada fase corresponde um Milestone no GitHub.

## **Fase 0 — Fundação (concluída)**

**Objetivo:** Servidor HTTP/1.1 sobre `io_uring`/monoio, thread-per-core, com roteamento por macros, estado e acesso a PostgreSQL.

- [x] **Estado da aplicação (App State)** (#8)
- [x] **Estudo de viabilidade: handlers `async` com injeção de estado em monoio** (#20)
- [x] **Executor SQL genérico via actor model (tokio-on-monoio)** (#22)
- [x] **Roteamento declarativo via macros de atributo** (#23)
- [x] **Buffer de conexão reutilizável** (#24)
- [x] **Trait `IntoResponse`** (#17)
- [x] **Suporte a headers e `Content-Type`** (#18)
- [x] **Helper de resposta JSON** (#13)
- [x] **Flexibilidade de argumentos em handlers via macro** (#29)

## **Fase 1 — Fundamentos HTTP/1.1**

**Objetivo:** Parser, leitura, escrita e roteamento corretos e seguros, com um baseline de benchmark para guiar as fases seguintes.

- [ ] **Suporte a Partial Reads** — P0 · M (#9)
- [ ] **Query Parameters e Roteamento Correto de URIs** — P0 · M (#10)
- [ ] **Suporte a Request Body** — P0 · M (#15)
- [ ] **Parser de Request sobre Bytes (sem UTF-8 global)** — P1 · M (#7)
- [ ] **Baseline de Benchmark Reproduzível** — P1 · M (#31)
- [ ] **PathTree com Backtracking e Detecção de Conflitos** — P1 · L (#32)
- [ ] **Limites de Request (URI, Headers, Body)** — P1 · M (#33)
- [ ] **405, `Allow`, OPTIONS Automático e HEAD** — P1 · M (#34)
- [ ] **Semântica de Conexão e Erros Não-Fatais** — P1 · M (#35)
- [ ] **Resposta em Bytes, `Date`/`Server` e Sanitização de Headers** — P1 · M (#36)
- [ ] **Chunked Transfer-Encoding e `Expect: 100-continue`** — P2 · M (#37)
- [ ] **Headers Repetidos** — P2 · S (#38)

## **Fase 2 — Handlers, State & Extractors**

**Objetivo:** Novo modelo de injeção de dependência no estilo axum (`State<S>`, `FromRef`, `FromRequest`), macros finas e extractors tipados.

- [ ] **Spike: Handler por Aridade com `Request<'a>` Emprestado** — P0 · M (#39)
- [ ] **`Handler` por Aridade e `Router<S>` sem `Option<Arc<T>>`** — P0 · L (#40)
- [ ] **`FromRequest`, `FromRef` e Extractor `State<S>`** — P0 · L (#41)
- [ ] **Estado por Worker (`with_state` e `with_state_factory`)** — P0 · M (#42)
- [ ] **Macros Finas de Rota** — P1 · M (#43)
- [ ] **`#[derive(FromRef)]` para o Estado** — P1 · S (#44)
- [ ] **Extractors `Path<T>` e `Query<T>`** — P1 · M (#46)
- [ ] **Extractors `Json<T>`, `Bytes`, `String` e `Headers`** — P1 · M (#47)
- [ ] **Handlers Retornando `Result` e `IntoResponse` Ampliado** — P1 · M (#48)
- [ ] **Guia de Extractors, Migração e Exemplo Atualizado** — P1 · S (#49)
- [ ] **Testes de Macros de Roteamento (`forge-macros`)** — P2 · S (#25)
- [ ] **Helper Opcional de Registro de Rotas** — P2 · S (#45)
- [ ] **Desacoplar `Response` do `monoio::TcpStream`** — P2 · M (#50)
- [ ] **Extensions por Request** — P3 · M (#51)

## **Fase 3 — Banco de Dados Modular**

**Objetivo:** Contratos sem `Send` obrigatório, pool, transações e adapter Postgres opcional. O usuário pode trazer o próprio driver.

- [ ] **Contratos de Banco sem `Send` Obrigatório** — P0 · L (#53)
- [ ] **Pool no Caminho Seguro** — P0 · L (#54)
- [ ] **Transações** — P0 · L (#55)
- [ ] **Adapter `forge-postgres`** — P1 · L (#56)
- [ ] **Integração com State e Múltiplos Bancos** — P1 · M (#57)
- [ ] **API de Query: `execute`, `FromRow`, Parâmetros Tipados** — P1 · L (#58)
- [ ] **Bug: Panic em `DbValue::decode_cell` e Tipos Desconhecidos** — P1 · S (#59)
- [ ] **Spike: `tokio-postgres` Nativo no Monoio (Pool por Worker)** — P2 · M (#52)
- [ ] **`MockDatabase` e Guia "Traga seu Driver"** — P2 · M (#60)
- [ ] **Guia: Usando `sqlx` (ou Outro Driver tokio-only)** — P2 · S (#61)
- [ ] **Feature Flags: `database` e `config` Opcionais** — P2 · S (#62)
- [ ] **TLS no Banco** — P3 · S (#63)
- [ ] **Runner de Migrations** — Icebox · L (#64)

## **Fase 4 — Performance**

**Objetivo:** Hot path sem alocação, escrita de resposta eficiente, pipelining e ajustes de io_uring, sempre medidos contra o baseline.

- [ ] **Headers e Params sem `HashMap`** — P1 · M (#65)
- [ ] **Medir e Eliminar Alocações por Request** — P1 · L (#66)
- [ ] **Escrita de Resposta sem `Vec` Intermediário** — P1 · M (#67)
- [ ] **Spike: `panic = "abort"` × `unwind` e Isolamento de Handler** — P1 · M (#70)
- [ ] **Pipelining HTTP/1.1 e Batch de Writes** — P2 · M (#68)
- [ ] **Spike: Ajustes de io_uring (Multishot Accept, Buffer Rings, SQPOLL)** — P2 · L (#69)
- [ ] **Gate de Regressão de Performance no CI** — P2 · M (#71)
- [ ] **`LruCache::touch` em O(1)** — P3 · S (#72)

## **Fase 5 — Router & Middleware**

**Objetivo:** Middleware, route groups, wildcards, arquivos estáticos, CORS e afins.

- [ ] **Middleware Core** — P1 · L (#73)
- [ ] **Suporte a Wildcards** — P2 · M (#11)
- [ ] **Route Groups / `nest` com Prefixo** — P2 · M (#74)
- [ ] **Static Files (`ServeDir`)** — P2 · L (#76)
- [ ] **Middleware CORS** — P2 · M (#77)
- [ ] **Registro de Rotas sem `panic!`** — P3 · S (#75)
- [ ] **Middleware de Compressão** — P3 · M (#78)
- [ ] **Middleware de Rate Limiting** — P3 · M (#79)

## **Fase 6 — Operação & Observabilidade**

**Objetivo:** Graceful shutdown, timeouts, logging estruturado, erros consistentes e health checks.

- [ ] **Graceful Shutdown** — P1 · L (#12)
- [ ] **Timeouts de Leitura, Idle e Handler** — P1 · M (#80)
- [ ] **Erros com Corpo, sem Vazar Internos, e Hierarquia Unificada** — P1 · M (#81)
- [ ] **Logging Estruturado (`tracing`) e Request-ID** — P1 · M (#82)
- [ ] **Access Log (Middleware de Logging)** — P2 · S (#14)
- [ ] **Limite de Conexões e Backpressure no Accept** — P2 · M (#83)
- [ ] **Config: `SocketAddr`/IPv6, `from_file` Integrado e Validação** — P2 · S (#84)
- [ ] **Health/Readiness e Métricas** — P3 · M (#85)

## **Fase 7 — Segurança & Protocolos**

**Objetivo:** TLS, cookies e auth, streaming/SSE, WebSockets, HTTP/2 e OpenAPI.

- [ ] **TLS (rustls) no Listener** — P1 · L (#86)
- [ ] **Cookies e Sessões** — P3 · L (#87)
- [ ] **Middleware de Autenticação (Bearer/Basic)** — P3 · M (#88)
- [ ] **Streaming de Respostas e SSE** — P3 · L (#89)
- [ ] **WebSockets** — P3 · XL (#90)
- [ ] **Validação de Input (derive)** — P3 · M (#92)
- [ ] **HTTP/2** — Icebox · XL (#91)
- [ ] **OpenAPI** — Icebox · XL (#93)

## **Fase 8 — Qualidade, Docs & Distribuição**

**Objetivo:** CI, testes, documentação, exemplos e processo de release.

- [ ] **README Correto e Quick Start** — P1 · S (#94)
- [ ] **CI Completo** — P1 · M (#95)
- [ ] **Testes Unitários Faltantes e Cobertura** — P2 · M (#96)
- [ ] **Testes de Integração E2E e Fuzz do Parser** — P2 · L (#97)
- [ ] **Exemplos em `examples/`** — P2 · M (#98)
- [ ] **Rustdoc da API Pública** — P2 · M (#99)
- [ ] **Templates e Governança do Repositório** — P2 · S (#100)
- [ ] **Metadados e Processo de Release** — P3 · M (#101)

---

Prioridades: `P0` crítico, `P1` alto, `P2` médio, `P3` baixo, `Icebox` sem previsão. Tamanhos: `XS` a `XL`.
