package configctl

import (
	"context"

	configdomain "internal/domain/configctl"
	"internal/shared/events"
	"internal/shared/problem"
)

type Repository interface {
	SaveConfigSet(context.Context, configdomain.ConfigSet) *problem.Problem
	DeleteConfigSet(context.Context, string) *problem.Problem
	GetConfigSetByID(context.Context, string) (configdomain.ConfigSet, *problem.Problem)
	GetConfigSetByKey(context.Context, string) (configdomain.ConfigSet, *problem.Problem)
	GetConfigSetByVersionID(context.Context, string) (configdomain.ConfigSet, *problem.Problem)
	ListConfigSets(context.Context) ([]configdomain.ConfigSet, *problem.Problem)
	SaveActivation(context.Context, configdomain.Activation) *problem.Problem
	DeleteActivation(context.Context, string) *problem.Problem
	GetActivationByScope(context.Context, configdomain.ActivationScope) (configdomain.Activation, *problem.Problem)
	ListActivationsByVersionID(context.Context, string) ([]configdomain.Activation, *problem.Problem)
	SaveIngestionRuntime(context.Context, configdomain.IngestionRuntimeProjection) *problem.Problem
	DeleteIngestionRuntimeByScope(context.Context, configdomain.ActivationScope) *problem.Problem
	ListIngestionRuntimes(context.Context) ([]configdomain.IngestionRuntimeProjection, *problem.Problem)
}

type DomainEventPublisher interface {
	Publish(context.Context, events.Event) *problem.Problem
}
