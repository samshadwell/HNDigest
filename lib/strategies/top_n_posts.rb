# frozen_string_literal: true

module Strategies
  class TopNPosts
    def initialize(num_posts)
      @n = num_posts
    end

    def type
      "TOP_N##{@n}"
    end

    def select(all_posts)
      all_posts.first(@n)
    end
  end
end
