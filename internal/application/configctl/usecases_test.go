package configctl

import (
	"context"
	"testing"
	"time"

	memoryrepo "internal/adapters/repositories/memory/configctl"
	"internal/application/configctl/contracts"
	configdomain "internal/domain/configctl"
	"internal/shared/events"
	"internal/shared/problem"
)

type domainEventPublisherSpy struct {
	events []events.Event
	prob   *problem.Problem
}

func (s *domainEventPublisherSpy) Publish(_ context.Context, event events.Event) *problem.Problem {
	s.events = append(s.events, event)
	return s.prob
}

func newTestRepository() *memoryrepo.Repository {
	return memoryrepo.NewRepository(nil)
}

func TestCreateDraftUseCaseCreatesNewDraftVersionAndPublishesDomainEvent(t *testing.T) {
	t.Parallel()

	repository := newTestRepository()
	publisher := &domainEventPublisherSpy{}
	useCase := NewCreateDraftUseCase(repository, publisher)
	useCase.now = func() time.Time { return time.Unix(10, 0).UTC() }
	ids := []string{"set-1", "ver-1", "ver-2"}
	useCase.nextID = func() string {
		value := ids[0]
		ids = ids[1:]
		return value
	}

	reply, prob := useCase.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: invalidDraftSource(),
	})
	if prob != nil {
		t.Fatalf("expected no problem, got %v", prob)
	}
	if reply.Config.ConfigSetID != "set-1" || reply.Config.ID != "ver-1" {
		t.Fatalf("expected set/version ids to be persisted, got %+v", reply.Config)
	}
	if reply.Config.Lifecycle != string(configdomain.LifecycleDraft) {
		t.Fatalf("expected lifecycle draft, got %q", reply.Config.Lifecycle)
	}
	if len(publisher.events) != 1 {
		t.Fatalf("expected one domain event, got %d", len(publisher.events))
	}
	if _, ok := publisher.events[0].(configdomain.DraftCreatedEvent); !ok {
		t.Fatalf("expected draft created event, got %T", publisher.events[0])
	}

	_, prob = useCase.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: invalidDraftSource(),
	})
	if prob == nil {
		t.Fatal("expected second open draft to be rejected")
	}
}

func TestCreateDraftUseCaseRollsBackWhenPublisherFails(t *testing.T) {
	t.Parallel()

	repository := newTestRepository()
	useCase := NewCreateDraftUseCase(repository, &domainEventPublisherSpy{
		prob: problem.New(problem.Unavailable, "jetstream unavailable"),
	})
	useCase.nextID = func() string { return "cfg-id" }

	_, prob := useCase.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: invalidDraftSource(),
	})
	if prob == nil {
		t.Fatal("expected problem")
	}
	sets, listProb := repository.ListConfigSets(context.Background())
	if listProb != nil {
		t.Fatalf("list config sets: %v", listProb)
	}
	if len(sets) != 0 {
		t.Fatal("expected repository rollback")
	}
}

func TestLifecycleUseCasesValidateCompileActivateDeactivateAndQuery(t *testing.T) {
	t.Parallel()

	repository := newTestRepository()
	publisher := &domainEventPublisherSpy{}
	create := NewCreateDraftUseCase(repository, publisher)
	validate := NewValidateConfigUseCase(repository, publisher)
	compile := NewCompileConfigUseCase(repository, publisher)
	activate := NewActivateConfigUseCase(repository, publisher)
	deactivate := NewDeactivateConfigUseCase(repository, publisher)
	get := NewGetConfigUseCase(repository)
	getActive := NewGetActiveConfigUseCase(repository)
	listIngestionBindings := NewListActiveIngestionBindingsUseCase(repository)
	list := NewListConfigsUseCase(repository)

	now := time.Unix(100, 0).UTC()
	create.now = func() time.Time { return now }
	create.nextID = sequence("set-1", "ver-1", "act-1")
	validate.now = func() time.Time { return now.Add(time.Minute) }
	compile.now = func() time.Time { return now.Add(2 * time.Minute) }
	activate.now = func() time.Time { return now.Add(3 * time.Minute) }
	activate.nextID = sequence("act-1")
	deactivate.now = func() time.Time { return now.Add(4 * time.Minute) }

	created, prob := create.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: validDraftSource(),
	})
	if prob != nil {
		t.Fatalf("create draft: %v", prob)
	}

	validated, prob := validate.Execute(context.Background(), contracts.ValidateConfigCommand{VersionID: created.Config.ID})
	if prob != nil {
		t.Fatalf("validate config: %v", prob)
	}
	if !validated.Valid || validated.Config.DefinitionChecksum == "" {
		t.Fatalf("expected validated config with checksum, got %+v", validated)
	}

	compiled, prob := compile.Execute(context.Background(), contracts.CompileConfigCommand{
		VersionID:     created.Config.ID,
		ArtifactID:    "artifact-1",
		SchemaVersion: "runtime/v1",
		Checksum:      "artifact-checksum",
		StorageRef:    "memory://artifacts/core/v1",
		RuntimeLoader: "validator:v1",
	})
	if prob != nil {
		t.Fatalf("compile config: %v", prob)
	}
	if compiled.Config.Artifact == nil || compiled.Config.Lifecycle != string(configdomain.LifecycleCompiled) {
		t.Fatalf("expected compiled artifact, got %+v", compiled.Config)
	}

	activated, prob := activate.Execute(context.Background(), contracts.ActivateConfigCommand{VersionID: created.Config.ID})
	if prob != nil {
		t.Fatalf("activate config: %v", prob)
	}
	if activated.Config.Lifecycle != "active" {
		t.Fatalf("expected active lifecycle, got %q", activated.Config.Lifecycle)
	}
	if activated.Projection.Artifact.ID != "artifact-1" {
		t.Fatalf("expected projection artifact id %q, got %q", "artifact-1", activated.Projection.Artifact.ID)
	}

	ingestionBindingsReply, prob := listIngestionBindings.Execute(context.Background(), contracts.ListActiveIngestionBindingsQuery{})
	if prob != nil {
		t.Fatalf("list active ingestion bindings: %v", prob)
	}
	if len(ingestionBindingsReply.Bindings) != 1 {
		t.Fatalf("expected one active ingestion binding, got %d", len(ingestionBindingsReply.Bindings))
	}
	if ingestionBindingsReply.Bindings[0].Binding.Topic != "orders.v1" {
		t.Fatalf("expected orders.v1 topic, got %+v", ingestionBindingsReply.Bindings[0])
	}
	if ingestionBindingsReply.Bindings[0].Runtime.Config.VersionID != created.Config.ID {
		t.Fatalf("expected runtime version id %q, got %+v", created.Config.ID, ingestionBindingsReply.Bindings[0].Runtime)
	}
	if ingestionBindingsReply.Bindings[0].Runtime.Config.DefinitionChecksum == "" {
		t.Fatalf("expected runtime definition checksum, got %+v", ingestionBindingsReply.Bindings[0].Runtime)
	}

	activeReply, prob := getActive.Execute(context.Background(), contracts.GetActiveConfigQuery{})
	if prob != nil {
		t.Fatalf("get active config: %v", prob)
	}
	if activeReply.Config.ID != created.Config.ID {
		t.Fatalf("expected active config id %q, got %q", created.Config.ID, activeReply.Config.ID)
	}

	getReply, prob := get.Execute(context.Background(), contracts.GetConfigQuery{VersionID: created.Config.ID})
	if prob != nil {
		t.Fatalf("get config: %v", prob)
	}
	if len(getReply.Config.ActiveScopes) != 1 {
		t.Fatalf("expected one active scope, got %d", len(getReply.Config.ActiveScopes))
	}

	listReply, prob := list.Execute(context.Background(), contracts.ListConfigsQuery{})
	if prob != nil {
		t.Fatalf("list configs: %v", prob)
	}
	if len(listReply.Configs) != 1 {
		t.Fatalf("expected one config version, got %d", len(listReply.Configs))
	}
	if listReply.Configs[0].ID != created.Config.ID || listReply.Configs[0].Artifact == nil {
		t.Fatalf("expected summarized version in list response, got %+v", listReply.Configs[0])
	}

	deactivated, prob := deactivate.Execute(context.Background(), contracts.DeactivateConfigCommand{})
	if prob != nil {
		t.Fatalf("deactivate config: %v", prob)
	}
	if deactivated.Config.Lifecycle != "inactive" {
		t.Fatalf("expected inactive lifecycle, got %q", deactivated.Config.Lifecycle)
	}

	ingestionBindingsReply, prob = listIngestionBindings.Execute(context.Background(), contracts.ListActiveIngestionBindingsQuery{})
	if prob != nil {
		t.Fatalf("list active ingestion bindings after deactivation: %v", prob)
	}
	if len(ingestionBindingsReply.Bindings) != 0 {
		t.Fatalf("expected no active ingestion bindings after deactivation, got %+v", ingestionBindingsReply.Bindings)
	}

	foundChangedEvent := false
	for _, event := range publisher.events {
		if changed, ok := event.(configdomain.IngestionRuntimeChangedEvent); ok {
			foundChangedEvent = true
			if changed.ChangeType == configdomain.IngestionRuntimeChangeActivated && changed.Runtime == nil {
				t.Fatalf("expected active ingestion runtime event to carry runtime snapshot")
			}
		}
	}
	if !foundChangedEvent {
		t.Fatal("expected ingestion runtime changed events to be published")
	}
}

func TestCompileUseCaseRejectsInvalidTransition(t *testing.T) {
	t.Parallel()

	repository := newTestRepository()
	create := NewCreateDraftUseCase(repository, nil)
	create.nextID = sequence("set-1", "ver-1")
	created, prob := create.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: validDraftSource(),
	})
	if prob != nil {
		t.Fatalf("create draft: %v", prob)
	}

	_, prob = NewCompileConfigUseCase(repository, nil).Execute(context.Background(), contracts.CompileConfigCommand{
		VersionID:     created.Config.ID,
		ArtifactID:    "artifact-1",
		SchemaVersion: "runtime/v1",
		Checksum:      "artifact-checksum",
		StorageRef:    "memory://artifacts/core/v1",
		RuntimeLoader: "validator:v1",
	})
	if prob == nil || prob.Code != problem.Conflict {
		t.Fatalf("expected conflict compiling draft, got %v", prob)
	}
}

func TestListActiveIngestionBindingsUseCaseRequiresCompleteScopeFilter(t *testing.T) {
	t.Parallel()

	_, prob := NewListActiveIngestionBindingsUseCase(newTestRepository()).Execute(context.Background(), contracts.ListActiveIngestionBindingsQuery{
		ScopeKind: "tenant",
	})
	if prob == nil || prob.Code != problem.InvalidArgument {
		t.Fatalf("expected invalid argument problem, got %v", prob)
	}
}

func TestCompileUseCaseBuildsDefaultArtifactMetadata(t *testing.T) {
	t.Parallel()

	repository := newTestRepository()
	create := NewCreateDraftUseCase(repository, nil)
	validate := NewValidateConfigUseCase(repository, nil)
	compile := NewCompileConfigUseCase(repository, nil)

	create.nextID = sequence("set-1", "ver-1")
	compile.nextID = sequence("artifact-1")
	created, prob := create.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: validDraftSource(),
	})
	if prob != nil {
		t.Fatalf("create draft: %v", prob)
	}
	if _, prob := validate.Execute(context.Background(), contracts.ValidateConfigCommand{VersionID: created.Config.ID}); prob != nil {
		t.Fatalf("validate config: %v", prob)
	}

	reply, prob := compile.Execute(context.Background(), contracts.CompileConfigCommand{VersionID: created.Config.ID})
	if prob != nil {
		t.Fatalf("compile config: %v", prob)
	}
	if reply.Config.Artifact == nil {
		t.Fatal("expected generated artifact")
	}
	if reply.Config.Artifact.ID != "artifact-1" {
		t.Fatalf("expected generated artifact id, got %q", reply.Config.Artifact.ID)
	}
	if reply.Config.Artifact.RuntimeLoader != "validator:v1" {
		t.Fatalf("expected default runtime loader, got %q", reply.Config.Artifact.RuntimeLoader)
	}
	if reply.Config.Artifact.StorageRef == "" || reply.Config.Artifact.Checksum == "" {
		t.Fatalf("expected generated artifact metadata, got %+v", reply.Config.Artifact)
	}
}

func TestActivateConfigUseCaseRollsBackWhenPublisherFails(t *testing.T) {
	t.Parallel()

	repository := newTestRepository()
	create := NewCreateDraftUseCase(repository, nil)
	validate := NewValidateConfigUseCase(repository, nil)
	compile := NewCompileConfigUseCase(repository, nil)
	activate := NewActivateConfigUseCase(repository, &domainEventPublisherSpy{
		prob: problem.New(problem.Unavailable, "event bus unavailable"),
	})

	now := time.Unix(200, 0).UTC()
	create.now = func() time.Time { return now }
	create.nextID = sequence("set-1", "ver-1")
	validate.now = func() time.Time { return now.Add(time.Minute) }
	compile.now = func() time.Time { return now.Add(2 * time.Minute) }
	activate.now = func() time.Time { return now.Add(3 * time.Minute) }
	activate.nextID = sequence("act-1")

	created, prob := create.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: validDraftSource(),
	})
	if prob != nil {
		t.Fatalf("create draft: %v", prob)
	}

	if _, prob := validate.Execute(context.Background(), contracts.ValidateConfigCommand{VersionID: created.Config.ID}); prob != nil {
		t.Fatalf("validate config: %v", prob)
	}
	if _, prob := compile.Execute(context.Background(), contracts.CompileConfigCommand{
		VersionID:     created.Config.ID,
		ArtifactID:    "artifact-1",
		SchemaVersion: "runtime/v1",
		Checksum:      "artifact-checksum",
		StorageRef:    "memory://artifacts/core/v1",
		RuntimeLoader: "validator:v1",
	}); prob != nil {
		t.Fatalf("compile config: %v", prob)
	}

	if _, prob := activate.Execute(context.Background(), contracts.ActivateConfigCommand{VersionID: created.Config.ID}); prob == nil {
		t.Fatal("expected publisher failure")
	}

	set, prob := repository.GetConfigSetByVersionID(context.Background(), created.Config.ID)
	if prob != nil {
		t.Fatalf("get config set: %v", prob)
	}
	version, ok := set.VersionByID(created.Config.ID)
	if !ok {
		t.Fatal("expected persisted version")
	}
	if version.Lifecycle != configdomain.LifecycleCompiled {
		t.Fatalf("expected activation rollback to restore compiled lifecycle, got %q", version.Lifecycle)
	}

	if _, prob := repository.GetActivationByScope(context.Background(), configdomain.DefaultActivationScope()); prob == nil {
		t.Fatal("expected activation rollback to remove active scope")
	}
}

func TestValidateDraftUseCaseReturnsDiagnosticsForInvalidDocument(t *testing.T) {
	t.Parallel()

	reply, prob := NewValidateDraftUseCase().Execute(context.Background(), contracts.ValidateDraftCommand{
		Format:  "json",
		Content: `{"metadata":{"name":""}}`,
	})
	if prob != nil {
		t.Fatalf("expected no transport problem, got %v", prob)
	}
	if reply.Valid || len(reply.Diagnostics) == 0 {
		t.Fatal("expected invalid reply")
	}
}

func sequence(values ...string) func() string {
	return func() string {
		value := values[0]
		values = values[1:]
		return value
	}
}

func validDraftSource() string {
	return `{
		"metadata":{"name":"Core Quality Config","description":"baseline quality checks"},
		"bindings":[{"name":"orders","topic":"orders.v1"}],
		"fields":[
			{"name":"order_id","type":"string","required":true},
			{"name":"status","type":"string","required":true}
		],
		"rules":[
			{"name":"order_id_required","field":"order_id","operator":"required","severity":"error"},
			{"name":"status_not_empty","field":"status","operator":"not_empty","severity":"error"}
		]
	}`
}

func invalidDraftSource() string {
	return `{"metadata":{"name":""}}`
}
