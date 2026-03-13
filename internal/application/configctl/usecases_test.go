package configctl

import (
	"context"
	"testing"
	"time"

	"internal/application/configctl/contracts"
	"internal/domain/configuration"
	"internal/shared/problem"
)

type repositoryStub struct {
	configs  map[string]configuration.Config
	order    []string
	activeID string
	version  int64
}

func newRepositoryStub() *repositoryStub {
	return &repositoryStub{
		configs: make(map[string]configuration.Config),
	}
}

func (s *repositoryStub) SaveDraft(_ context.Context, config configuration.Config) *problem.Problem {
	s.configs[config.ID] = config
	s.order = append(s.order, config.ID)
	s.version++
	return nil
}

func (s *repositoryStub) Delete(_ context.Context, id string) *problem.Problem {
	delete(s.configs, id)
	s.version++
	return nil
}

func (s *repositoryStub) GetByID(_ context.Context, id string) (configuration.Config, *problem.Problem) {
	config, ok := s.configs[id]
	if !ok {
		return configuration.Config{}, problem.New(problem.NotFound, "config not found")
	}
	return config, nil
}

func (s *repositoryStub) GetActive(_ context.Context) (configuration.Config, *problem.Problem) {
	if s.activeID == "" {
		return configuration.Config{}, problem.New(problem.NotFound, "active config not found")
	}
	return s.GetByID(context.Background(), s.activeID)
}

func (s *repositoryStub) List(_ context.Context) ([]configuration.Config, *problem.Problem) {
	configs := make([]configuration.Config, 0, len(s.order))
	for _, id := range s.order {
		config, ok := s.configs[id]
		if ok {
			configs = append(configs, config)
		}
	}
	return configs, nil
}

func (s *repositoryStub) Snapshot(ctx context.Context) (contracts.RuntimeSnapshot, *problem.Problem) {
	configs, prob := s.List(ctx)
	if prob != nil {
		return contracts.RuntimeSnapshot{}, prob
	}

	snapshot := contracts.RuntimeSnapshot{
		Version:        s.version,
		ActiveConfigID: s.activeID,
		Configs:        make([]contracts.ConfigRecord, 0, len(configs)),
	}
	for _, config := range configs {
		snapshot.Configs = append(snapshot.Configs, recordFromDomain(config))
	}
	return snapshot, nil
}

type runtimePublisherSpy struct {
	events []contracts.RuntimeEvent
	prob   *problem.Problem
}

func (s *runtimePublisherSpy) Publish(_ context.Context, event contracts.RuntimeEvent) *problem.Problem {
	s.events = append(s.events, event)
	return s.prob
}

func TestCreateDraftUseCasePublishesRuntimeSnapshot(t *testing.T) {
	t.Parallel()

	repository := newRepositoryStub()
	publisher := &runtimePublisherSpy{}
	useCase := NewCreateDraftUseCase(repository, publisher)
	useCase.now = func() time.Time { return time.Unix(10, 0).UTC() }
	useCase.nextID = func() string { return "cfg-123" }

	reply, prob := useCase.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: "{}",
	})
	if prob != nil {
		t.Fatalf("expected no problem, got %v", prob)
	}

	if reply.Config.ID != "cfg-123" {
		t.Fatalf("expected config id %q, got %q", "cfg-123", reply.Config.ID)
	}

	if len(publisher.events) != 1 {
		t.Fatalf("expected a runtime event, got %d", len(publisher.events))
	}

	event, ok := publisher.events[0].(contracts.RuntimeUpdatedEvent)
	if !ok {
		t.Fatalf("expected runtime updated event, got %T", publisher.events[0])
	}

	if event.Snapshot.Version != 1 {
		t.Fatalf("expected snapshot version 1, got %d", event.Snapshot.Version)
	}
}

func TestCreateDraftUseCaseRollsBackWhenPublisherFails(t *testing.T) {
	t.Parallel()

	repository := newRepositoryStub()
	useCase := NewCreateDraftUseCase(repository, &runtimePublisherSpy{
		prob: problem.New(problem.Unavailable, "jetstream unavailable"),
	})
	useCase.nextID = func() string { return "cfg-123" }

	_, prob := useCase.Execute(context.Background(), contracts.CreateDraftCommand{
		Name:    "core",
		Format:  "json",
		Content: "{}",
	})
	if prob == nil {
		t.Fatal("expected problem")
	}

	if _, exists := repository.configs["cfg-123"]; exists {
		t.Fatal("expected repository rollback")
	}
}

func TestGetAndListConfigUseCases(t *testing.T) {
	t.Parallel()

	repository := newRepositoryStub()
	config, prob := configuration.NewDraft("cfg-123", "core", configuration.FormatJSON, "{}", time.Unix(10, 0).UTC())
	if prob != nil {
		t.Fatalf("build config: %v", prob)
	}
	_ = repository.SaveDraft(context.Background(), config)

	getReply, prob := NewGetConfigUseCase(repository).Execute(context.Background(), contracts.GetConfigQuery{ID: "cfg-123"})
	if prob != nil {
		t.Fatalf("get config: %v", prob)
	}

	if getReply.Config.Name != "core" {
		t.Fatalf("expected config name %q, got %q", "core", getReply.Config.Name)
	}

	listReply, prob := NewListConfigsUseCase(repository).Execute(context.Background(), contracts.ListConfigsQuery{})
	if prob != nil {
		t.Fatalf("list configs: %v", prob)
	}

	if len(listReply.Configs) != 1 {
		t.Fatalf("expected 1 config, got %d", len(listReply.Configs))
	}
}

func TestValidateDraftUseCaseValidatesFormatSpecificContent(t *testing.T) {
	t.Parallel()

	reply, prob := NewValidateDraftUseCase().Execute(context.Background(), contracts.ValidateDraftCommand{
		Format:  "json",
		Content: "{",
	})
	if prob != nil {
		t.Fatalf("expected no transport problem, got %v", prob)
	}
	if reply.Valid || len(reply.Diagnostics) == 0 {
		t.Fatal("expected invalid reply")
	}
}
