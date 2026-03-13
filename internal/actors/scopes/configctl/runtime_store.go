package configctl

import (
	"context"
	"sort"

	configapp "internal/application/configctl"
	"internal/application/configctl/contracts"
	"internal/domain/configuration"
	"internal/shared/problem"
)

type runtimeStore struct {
	configs  map[string]configuration.Config
	order    []string
	activeID string
	version  int64
}

var _ configapp.Repository = (*runtimeStore)(nil)

func newRuntimeStore() *runtimeStore {
	return &runtimeStore{
		configs: make(map[string]configuration.Config),
	}
}

func (s *runtimeStore) SaveDraft(_ context.Context, config configuration.Config) *problem.Problem {
	if s == nil {
		return problem.New(problem.Unavailable, "runtime store is unavailable")
	}

	if _, exists := s.configs[config.ID]; !exists {
		s.order = append(s.order, config.ID)
	}

	s.configs[config.ID] = config
	s.version++
	return nil
}

func (s *runtimeStore) Delete(_ context.Context, id string) *problem.Problem {
	if s == nil {
		return problem.New(problem.Unavailable, "runtime store is unavailable")
	}

	delete(s.configs, id)
	s.version++
	return nil
}

func (s *runtimeStore) GetByID(_ context.Context, id string) (configuration.Config, *problem.Problem) {
	if s == nil {
		return configuration.Config{}, problem.New(problem.Unavailable, "runtime store is unavailable")
	}

	config, ok := s.configs[id]
	if !ok {
		return configuration.Config{}, problem.New(problem.NotFound, "config not found")
	}

	return config, nil
}

func (s *runtimeStore) GetActive(ctx context.Context) (configuration.Config, *problem.Problem) {
	if s == nil {
		return configuration.Config{}, problem.New(problem.Unavailable, "runtime store is unavailable")
	}
	if s.activeID == "" {
		return configuration.Config{}, problem.New(problem.NotFound, "active config not found")
	}
	return s.GetByID(ctx, s.activeID)
}

func (s *runtimeStore) List(_ context.Context) ([]configuration.Config, *problem.Problem) {
	if s == nil {
		return nil, problem.New(problem.Unavailable, "runtime store is unavailable")
	}

	configs := make([]configuration.Config, 0, len(s.order))
	for _, id := range s.order {
		config, ok := s.configs[id]
		if ok {
			configs = append(configs, config)
		}
	}

	sort.SliceStable(configs, func(i, j int) bool {
		return configs[i].CreatedAt.Before(configs[j].CreatedAt)
	})

	return configs, nil
}

func (s *runtimeStore) Snapshot(ctx context.Context) (contracts.RuntimeSnapshot, *problem.Problem) {
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
		snapshot.Configs = append(snapshot.Configs, contracts.ConfigRecord{
			ID:        config.ID,
			Name:      config.Name,
			Format:    string(config.Format),
			Content:   config.Content,
			Status:    string(config.Status),
			CreatedAt: config.CreatedAt,
			UpdatedAt: config.UpdatedAt,
		})
	}

	return snapshot, nil
}
