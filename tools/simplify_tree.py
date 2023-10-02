"""Print a simplified treeview of the i3 container tree"""
import json
import subprocess

from typing import TypedDict


class Node(TypedDict):
    """Simplified view of the node"""

    id: str
    name: str
    type: str
    nodes: list["Node"]


def only_interesting(node: Node) -> Node:
    """Reduce a node to only the interesting attributes"""
    return {
        "nodes": [only_interesting(p) for p in node.get("nodes", [])],
        "id": node.get("id", None),
        "name": node.get("name", None),
        "type": node.get("type", None),
    }


def print_tree(tree: Node, indent_str: str = "  ") -> str:
    """Print a tree in a one-line indented fashion"""

    def _internal_print_tree(node: Node, depth: int) -> str:
        result_list = [
            indent_str * depth + print_node(node),
            *[_internal_print_tree(n, depth + 1) for n in node["nodes"]],
        ]
        return "\n".join(result_list)

    def print_node(node: Node) -> str:
        return f"{node['type']}: {node['name']} [{node['id']}]"

    return _internal_print_tree(tree, 0)


def main():
    """Main function"""
    process = subprocess.run(
        ["i3-msg", "-t", "get_tree"],
        stdout=subprocess.PIPE,
        check=True,
        encoding="utf-8",
    )
    tree = json.loads(process.stdout)
    print(print_tree(tree))
    # print(json.dumps(only_interesting(tree), indent=2))


if __name__ == "__main__":
    main()
