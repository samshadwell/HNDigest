# frozen_string_literal: true

module Strategies
  class OverPointThreshold
    def initialize(point_threshold)
      @point_threshold = point_threshold
    end

    def type
      "POINT_THRESHOLD##{@point_threshold}"
    end

    def select(all_posts)
      all_posts.select { |post| post['points'] >= @point_threshold }
    end
  end
end
