# frozen_string_literal: true

require_relative '../configuration'
require_relative 'strategies/over_point_threshold'
require_relative 'strategies/top_n_posts'

class StrategyFactory
  def self.all_strategies
    strategies = Configuration::TOP_N_VALUES.map do |n|
      Strategies::TopNPosts.new(n)
    end

    Configuration::POINT_THRESHOLD_VALUES.each do |threshold|
      strategies << Strategies::OverPointThreshold.new(threshold)
    end

    strategies
  end
end
