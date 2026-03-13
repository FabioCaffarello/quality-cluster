package configctl

import (
	"context"

	"internal/application/configctl/contracts"
	"internal/domain/configuration"
	"internal/shared/problem"
)

type Repository interface {
	SaveDraft(context.Context, configuration.Config) *problem.Problem
	Delete(context.Context, string) *problem.Problem
	GetByID(context.Context, string) (configuration.Config, *problem.Problem)
	GetActive(context.Context) (configuration.Config, *problem.Problem)
	List(context.Context) ([]configuration.Config, *problem.Problem)
	Snapshot(context.Context) (contracts.RuntimeSnapshot, *problem.Problem)
}

type RuntimeEventPublisher interface {
	Publish(context.Context, contracts.RuntimeEvent) *problem.Problem
}
